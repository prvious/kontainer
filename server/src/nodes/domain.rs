//! Node membership in the controller's own terms, distinct from protobuf and persistence.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use kontainer_engine::{SwarmNode, SwarmNodeState, SwarmRole};

/// The cadence the controller asks agents to check in at.
pub const CHECK_IN_INTERVAL_SECONDS: u32 = 5;
/// `online` while the last check-in is within three intervals.
pub const ONLINE_WITHIN_SECONDS: i64 = 3 * CHECK_IN_INTERVAL_SECONDS as i64;
/// `degraded` until a minute without a check-in, then `offline`.
pub const DEGRADED_WITHIN_SECONDS: i64 = 60;

const MAX_TEXT_LEN: usize = 253;
const MAX_VERSION_LEN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    Online,
    Degraded,
    Offline,
}

impl NodeStatus {
    /// Status from the database clock at read time; never stored.
    #[must_use]
    pub fn at(last_check_in_at: DateTime<Utc>, now: DateTime<Utc>) -> Self {
        let silent_for = (now - last_check_in_at).num_milliseconds();
        if silent_for <= ONLINE_WITHIN_SECONDS * 1000 {
            Self::Online
        } else if silent_for <= DEGRADED_WITHIN_SECONDS * 1000 {
            Self::Degraded
        } else {
            Self::Offline
        }
    }
}

/// A validated check-in. `node_id` comes from the certificate, never the message.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckIn {
    pub node_id: String,
    pub agent_version: String,
    pub architecture: String,
    pub advertise_address: String,
    pub hostname: String,
    pub swarm_node_id: Option<String>,
    pub docker_version: String,
    pub os: String,
    pub cpu_cores: u32,
    pub memory_total_bytes: u64,
    pub agent_started_at: DateTime<Utc>,
    pub sample: Option<SampleReading>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampleReading {
    pub sampled_at: DateTime<Utc>,
    pub cpu_percent: f32,
    pub memory_used_bytes: u64,
    pub disk_used_bytes: Option<u64>,
    pub containers_running: u32,
    pub containers_paused: u32,
    pub containers_exited: u32,
    pub net_rx_bytes_per_sec: Option<f64>,
    pub net_tx_bytes_per_sec: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CheckInRejection {
    #[error("advertise_address must be host:port")]
    AdvertiseAddress,
    #[error("a text field is empty or too long")]
    Text,
    #[error("a sample figure is out of range")]
    Sample,
}

/// Bounds on what an agent may claim. Text is trimmed-length-checked; figures must be finite and
/// non-negative, CPU within 0–100.
pub fn validate(check_in: &CheckIn) -> Result<(), CheckInRejection> {
    if !valid_host_port(&check_in.advertise_address) {
        return Err(CheckInRejection::AdvertiseAddress);
    }
    for (value, limit, required) in [
        (check_in.hostname.as_str(), MAX_TEXT_LEN, false),
        (check_in.agent_version.as_str(), MAX_VERSION_LEN, true),
        (check_in.architecture.as_str(), MAX_VERSION_LEN, true),
        (check_in.docker_version.as_str(), MAX_VERSION_LEN, false),
        (check_in.os.as_str(), MAX_TEXT_LEN, false),
        (
            check_in.swarm_node_id.as_deref().unwrap_or_default(),
            MAX_VERSION_LEN,
            false,
        ),
    ] {
        if value.len() > limit || (required && value.trim().is_empty()) || value.contains('\0') {
            return Err(CheckInRejection::Text);
        }
    }
    if let Some(sample) = &check_in.sample {
        let rate_ok = |rate: Option<f64>| rate.is_none_or(|rate| rate.is_finite() && rate >= 0.0);
        if !sample.cpu_percent.is_finite()
            || !(0.0..=100.0).contains(&sample.cpu_percent)
            || !rate_ok(sample.net_rx_bytes_per_sec)
            || !rate_ok(sample.net_tx_bytes_per_sec)
            || i64::try_from(sample.memory_used_bytes).is_err()
            || sample
                .disk_used_bytes
                .is_some_and(|disk| i64::try_from(disk).is_err())
        {
            return Err(CheckInRejection::Sample);
        }
    }
    if i64::try_from(check_in.memory_total_bytes).is_err()
        || i32::try_from(check_in.cpu_cores).is_err()
    {
        return Err(CheckInRejection::Sample);
    }
    Ok(())
}

/// `host:port` with a DNS or IP host and a non-zero port.
#[must_use]
pub fn valid_host_port(value: &str) -> bool {
    let Some((host, port)) = value.rsplit_once(':') else {
        return false;
    };
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    !host.is_empty()
        && host.len() <= MAX_TEXT_LEN
        && host
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".-:".contains(character))
        && port.parse::<u16>().is_ok_and(|port| port > 0)
}

/// Latest sample as the node list shows it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatestSample {
    pub sampled_at: DateTime<Utc>,
    pub cpu_percent: f32,
    pub memory_used_bytes: i64,
    pub disk_used_bytes: Option<i64>,
    pub containers_running: i32,
    pub containers_paused: i32,
    pub containers_exited: i32,
    pub net_rx_bytes_per_sec: Option<f64>,
    pub net_tx_bytes_per_sec: Option<f64>,
}

/// One membership row with its derived status.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeView {
    pub id: String,
    pub hostname: String,
    pub status: NodeStatus,
    pub swarm_node_id: Option<String>,
    pub advertise_address: String,
    pub agent_version: String,
    pub docker_version: String,
    pub os: String,
    pub arch: String,
    pub cpu_cores: i32,
    pub memory_total_bytes: i64,
    pub agent_started_at: DateTime<Utc>,
    pub first_seen_at: DateTime<Utc>,
    pub last_check_in_at: DateTime<Utc>,
    pub latest_sample: Option<LatestSample>,
}

/// The agent side of a cluster node; `NoAgent` for a Swarm member that never checked in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Online,
    Degraded,
    Offline,
    NoAgent,
}

impl From<NodeStatus> for AgentStatus {
    fn from(status: NodeStatus) -> Self {
        match status {
            NodeStatus::Online => Self::Online,
            NodeStatus::Degraded => Self::Degraded,
            NodeStatus::Offline => Self::Offline,
        }
    }
}

/// A node's Swarm role; `Unknown` when the manager did not name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeRole {
    Manager,
    Worker,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeHealth {
    Active,
    Degraded,
    Down,
}

impl NodeHealth {
    /// `down` when either side says so, `active` only when both are fine, `degraded` otherwise.
    #[must_use]
    pub fn derive(agent: AgentStatus, swarm: SwarmNodeState) -> Self {
        match (agent, swarm) {
            (AgentStatus::Offline, _) | (_, SwarmNodeState::Down) => Self::Down,
            (AgentStatus::Online, SwarmNodeState::Ready) => Self::Active,
            _ => Self::Degraded,
        }
    }
}

/// A membership row joined to the manager's view of the same node; either side may be missing,
/// never both.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterNode {
    pub agent: Option<NodeView>,
    pub swarm: Option<SwarmNode>,
}

impl ClusterNode {
    #[must_use]
    pub fn agent_status(&self) -> AgentStatus {
        self.agent
            .as_ref()
            .map_or(AgentStatus::NoAgent, |agent| agent.status.into())
    }

    #[must_use]
    pub fn role(&self) -> NodeRole {
        match self.swarm.as_ref().map(|swarm| swarm.role) {
            Some(SwarmRole::Manager) => NodeRole::Manager,
            Some(SwarmRole::Worker) => NodeRole::Worker,
            None => NodeRole::Unknown,
        }
    }

    #[must_use]
    pub fn swarm_state(&self) -> SwarmNodeState {
        self.swarm
            .as_ref()
            .map_or(SwarmNodeState::Unknown, |swarm| swarm.state)
    }

    #[must_use]
    pub fn health(&self) -> NodeHealth {
        NodeHealth::derive(self.agent_status(), self.swarm_state())
    }

    /// The agent's hostname, else Swarm's, else whichever id there is.
    #[must_use]
    pub fn name(&self) -> &str {
        let agent = self
            .agent
            .as_ref()
            .map(|agent| agent.hostname.as_str())
            .filter(|name| !name.is_empty());
        let swarm = self
            .swarm
            .as_ref()
            .map(|swarm| swarm.hostname.as_str())
            .filter(|name| !name.is_empty());
        agent
            .or(swarm)
            .or(self.agent.as_ref().map(|agent| agent.id.as_str()))
            .or(self.swarm.as_ref().map(|swarm| swarm.id.as_str()))
            .unwrap_or_default()
    }

    /// The latest sample, counted only while the agent is online.
    fn online_sample(&self) -> Option<(&NodeView, &LatestSample)> {
        let agent = self.agent.as_ref()?;
        let sample = agent.latest_sample.as_ref()?;
        (agent.status == NodeStatus::Online).then_some((agent, sample))
    }
}

/// Pair each agent with the Swarm node its check-in names, then add Swarm members without an
/// agent. Ordered leader, managers, workers, unknown, then by name.
#[must_use]
pub fn join_nodes(agents: Vec<NodeView>, swarm: Vec<SwarmNode>) -> Vec<ClusterNode> {
    let mut swarm: BTreeMap<String, SwarmNode> = swarm
        .into_iter()
        .map(|node| (node.id.clone(), node))
        .collect();
    let mut joined: Vec<ClusterNode> = agents
        .into_iter()
        .map(|agent| ClusterNode {
            swarm: agent.swarm_node_id.as_ref().and_then(|id| swarm.remove(id)),
            agent: Some(agent),
        })
        .collect();
    joined.extend(swarm.into_values().map(|node| ClusterNode {
        agent: None,
        swarm: Some(node),
    }));
    joined.sort_by(|left, right| {
        let leader = |node: &ClusterNode| !node.swarm.as_ref().is_some_and(|swarm| swarm.is_leader);
        (leader(left), left.role(), left.name()).cmp(&(leader(right), right.role(), right.name()))
    });
    joined
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuorumState {
    Healthy,
    Degraded,
}

/// The cluster load strip, the Overview, and the status bar read these.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClusterSummary {
    pub nodes_total: u32,
    pub nodes_active: u32,
    pub nodes_degraded: u32,
    pub nodes_down: u32,
    pub managers_total: u32,
    pub managers_reachable: u32,
    pub quorum: QuorumState,
    /// Mean over online nodes with a sample; `None` when there are none.
    pub cpu_percent: Option<f64>,
    /// Summed over the same nodes.
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub containers_running: u64,
}

/// Counts by health, manager reachability, and load over online agents. Quorum is healthy when
/// the manager answered and every manager is ready and reachable.
#[must_use]
pub fn summarize(nodes: &[ClusterNode], swarm_answered: bool) -> ClusterSummary {
    let count = |predicate: &dyn Fn(&ClusterNode) -> bool| {
        u32::try_from(nodes.iter().filter(|node| predicate(node)).count()).unwrap_or(u32::MAX)
    };
    let managers = || {
        nodes
            .iter()
            .filter_map(|node| node.swarm.as_ref())
            .filter(|swarm| swarm.role == SwarmRole::Manager)
    };
    let managers_total = u32::try_from(managers().count()).unwrap_or(u32::MAX);
    let managers_reachable = u32::try_from(
        managers()
            .filter(|swarm| swarm.manager_reachable == Some(true))
            .count(),
    )
    .unwrap_or(u32::MAX);
    let every_manager_ready = managers()
        .all(|swarm| swarm.state == SwarmNodeState::Ready && swarm.manager_reachable == Some(true));
    let sampled: Vec<_> = nodes
        .iter()
        .filter_map(ClusterNode::online_sample)
        .collect();
    let cpu_percent = (!sampled.is_empty()).then(|| {
        let total: f64 = sampled
            .iter()
            .map(|(_, sample)| f64::from(sample.cpu_percent))
            .sum();
        #[allow(clippy::cast_precision_loss)]
        let mean = total / sampled.len() as f64;
        mean
    });
    let unsigned = |value: i64| u64::try_from(value).unwrap_or(0);
    ClusterSummary {
        nodes_total: count(&|_| true),
        nodes_active: count(&|node| node.health() == NodeHealth::Active),
        nodes_degraded: count(&|node| node.health() == NodeHealth::Degraded),
        nodes_down: count(&|node| node.health() == NodeHealth::Down),
        managers_total,
        managers_reachable,
        quorum: if swarm_answered && managers_total > 0 && every_manager_ready {
            QuorumState::Healthy
        } else {
            QuorumState::Degraded
        },
        cpu_percent,
        memory_used_bytes: sampled
            .iter()
            .map(|(_, sample)| unsigned(sample.memory_used_bytes))
            .sum(),
        memory_total_bytes: sampled
            .iter()
            .map(|(agent, _)| unsigned(agent.memory_total_bytes))
            .sum(),
        containers_running: sampled
            .iter()
            .map(|(_, sample)| unsigned(i64::from(sample.containers_running)))
            .sum(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};
    use kontainer_engine::{SwarmNode, SwarmNodeState, SwarmRole};

    use super::{
        CheckIn, CheckInRejection, NodeStatus, NodeView, SampleReading, valid_host_port, validate,
    };

    #[test]
    fn status_boundaries_are_fifteen_and_sixty_seconds() {
        let now = Utc.with_ymd_and_hms(2026, 9, 28, 12, 0, 0).unwrap();
        for (silent, expected) in [
            (0, NodeStatus::Online),
            (15, NodeStatus::Online),
            (16, NodeStatus::Degraded),
            (60, NodeStatus::Degraded),
            (61, NodeStatus::Offline),
            (86_400, NodeStatus::Offline),
        ] {
            assert_eq!(
                NodeStatus::at(now - Duration::seconds(silent), now),
                expected,
                "{silent}s"
            );
        }
        assert_eq!(
            NodeStatus::at(now + Duration::seconds(3), now),
            NodeStatus::Online,
            "a check-in slightly ahead of the database clock is online"
        );
    }

    fn check_in() -> CheckIn {
        CheckIn {
            node_id: "manager".to_owned(),
            agent_version: "0.1.0".to_owned(),
            architecture: "aarch64".to_owned(),
            advertise_address: "127.0.0.1:50052".to_owned(),
            hostname: "manager".to_owned(),
            swarm_node_id: Some("tj42rq5nzwf8".to_owned()),
            docker_version: "29.8.0".to_owned(),
            os: "Alpine Linux".to_owned(),
            cpu_cores: 4,
            memory_total_bytes: 8 << 30,
            agent_started_at: Utc::now(),
            sample: Some(SampleReading {
                sampled_at: Utc::now(),
                cpu_percent: 12.5,
                memory_used_bytes: 1 << 30,
                disk_used_bytes: None,
                containers_running: 3,
                containers_paused: 0,
                containers_exited: 1,
                net_rx_bytes_per_sec: Some(10.0),
                net_tx_bytes_per_sec: None,
            }),
        }
    }

    #[test]
    fn check_in_claims_are_bounded() {
        assert!(validate(&check_in()).is_ok());

        let mut bad = check_in();
        bad.advertise_address = "no-port".to_owned();
        assert_eq!(validate(&bad), Err(CheckInRejection::AdvertiseAddress));

        let mut bad = check_in();
        bad.hostname = "h".repeat(254);
        assert_eq!(validate(&bad), Err(CheckInRejection::Text));

        let mut bad = check_in();
        bad.agent_version = " ".to_owned();
        assert_eq!(validate(&bad), Err(CheckInRejection::Text));

        for cpu in [f32::NAN, -1.0, 100.5, f32::INFINITY] {
            let mut bad = check_in();
            bad.sample.as_mut().unwrap().cpu_percent = cpu;
            assert_eq!(validate(&bad), Err(CheckInRejection::Sample), "{cpu}");
        }
        let mut bad = check_in();
        bad.sample.as_mut().unwrap().net_tx_bytes_per_sec = Some(-5.0);
        assert_eq!(validate(&bad), Err(CheckInRejection::Sample));
        let mut bad = check_in();
        bad.memory_total_bytes = u64::MAX;
        assert_eq!(validate(&bad), Err(CheckInRejection::Sample));
    }

    #[test]
    fn host_ports_accept_names_and_addresses_only() {
        for valid in ["127.0.0.1:50052", "node-1.internal:1", "[::1]:50052"] {
            assert!(valid_host_port(valid), "{valid}");
        }
        for invalid in [
            "",
            ":50052",
            "host",
            "host:0",
            "host:99999",
            "bad host:1",
            "a/b:1",
        ] {
            assert!(!valid_host_port(invalid), "{invalid}");
        }
    }

    fn agent(id: &str, status: NodeStatus, swarm: Option<&str>, cpu: f32, memory: i64) -> NodeView {
        let at = Utc.with_ymd_and_hms(2026, 9, 28, 12, 0, 0).unwrap();
        NodeView {
            id: id.to_owned(),
            hostname: id.to_owned(),
            status,
            swarm_node_id: swarm.map(str::to_owned),
            advertise_address: "127.0.0.1:50052".to_owned(),
            agent_version: "0.1.0".to_owned(),
            docker_version: "29.1.0".to_owned(),
            os: "Alpine Linux".to_owned(),
            arch: "aarch64".to_owned(),
            cpu_cores: 4,
            memory_total_bytes: 8 << 30,
            agent_started_at: at,
            first_seen_at: at,
            last_check_in_at: at,
            latest_sample: Some(super::LatestSample {
                sampled_at: at,
                cpu_percent: cpu,
                memory_used_bytes: memory,
                disk_used_bytes: None,
                containers_running: 3,
                containers_paused: 0,
                containers_exited: 0,
                net_rx_bytes_per_sec: None,
                net_tx_bytes_per_sec: None,
            }),
        }
    }

    fn member(id: &str, hostname: &str, role: SwarmRole, state: SwarmNodeState) -> SwarmNode {
        SwarmNode {
            id: id.to_owned(),
            hostname: hostname.to_owned(),
            role,
            is_leader: false,
            manager_reachable: (role == SwarmRole::Manager).then_some(true),
            state,
            ..SwarmNode::default()
        }
    }

    #[test]
    fn health_is_down_when_either_side_says_so_and_active_only_when_both_are_fine() {
        use super::{AgentStatus as Agent, NodeHealth as Health};
        use SwarmNodeState::{Down, Ready, Unknown};

        for (agent, swarm, expected) in [
            (Agent::Online, Ready, Health::Active),
            (Agent::Online, Unknown, Health::Degraded),
            (Agent::Online, Down, Health::Down),
            (Agent::Degraded, Ready, Health::Degraded),
            (Agent::Degraded, Unknown, Health::Degraded),
            (Agent::Degraded, Down, Health::Down),
            (Agent::Offline, Ready, Health::Down),
            (Agent::Offline, Unknown, Health::Down),
            (Agent::NoAgent, Ready, Health::Degraded),
            (Agent::NoAgent, Down, Health::Down),
        ] {
            assert_eq!(
                Health::derive(agent, swarm),
                expected,
                "{agent:?} + {swarm:?}"
            );
        }
    }

    #[test]
    fn join_pairs_agents_by_swarm_id_and_keeps_both_orphans() {
        use super::{AgentStatus, NodeRole, join_nodes};

        let mut leader = member("mgr", "manager", SwarmRole::Manager, SwarmNodeState::Ready);
        leader.is_leader = true;
        let joined = join_nodes(
            vec![
                agent("standalone", NodeStatus::Online, None, 10.0, 1),
                agent("worker-1", NodeStatus::Online, Some("wk1"), 10.0, 1),
                agent("manager", NodeStatus::Online, Some("mgr"), 10.0, 1),
                agent("stale-id", NodeStatus::Online, Some("gone"), 10.0, 1),
            ],
            vec![
                member("wk2", "worker-2", SwarmRole::Worker, SwarmNodeState::Ready),
                member("wk1", "worker-1", SwarmRole::Worker, SwarmNodeState::Ready),
                leader,
            ],
        );
        let summary: Vec<_> = joined
            .iter()
            .map(|node| (node.name(), node.role(), node.agent_status()))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("manager", NodeRole::Manager, AgentStatus::Online),
                ("worker-1", NodeRole::Worker, AgentStatus::Online),
                ("worker-2", NodeRole::Worker, AgentStatus::NoAgent),
                ("stale-id", NodeRole::Unknown, AgentStatus::Online),
                ("standalone", NodeRole::Unknown, AgentStatus::Online),
            ],
            "leader first, then managers, workers, and agents Swarm does not know"
        );
        assert_eq!(
            joined[1].swarm.as_ref().map(|swarm| swarm.id.as_str()),
            Some("wk1")
        );
        assert!(joined[2].agent.is_none());
        assert_eq!(
            joined[3].swarm_state(),
            SwarmNodeState::Unknown,
            "an agent naming a Swarm node the manager does not list"
        );
    }

    #[test]
    fn summary_counts_health_managers_and_load_over_online_agents() {
        use super::{QuorumState, join_nodes, summarize};

        let mut leader = member("mgr", "manager", SwarmRole::Manager, SwarmNodeState::Ready);
        leader.is_leader = true;
        let nodes = join_nodes(
            vec![
                agent("manager", NodeStatus::Online, Some("mgr"), 30.0, 2 << 30),
                agent("worker-1", NodeStatus::Online, Some("wk1"), 10.0, 1 << 30),
                agent("worker-2", NodeStatus::Degraded, Some("wk2"), 90.0, 4 << 30),
                agent("worker-3", NodeStatus::Offline, Some("wk3"), 90.0, 4 << 30),
            ],
            vec![
                leader.clone(),
                member("wk1", "worker-1", SwarmRole::Worker, SwarmNodeState::Ready),
                member("wk2", "worker-2", SwarmRole::Worker, SwarmNodeState::Ready),
                member("wk3", "worker-3", SwarmRole::Worker, SwarmNodeState::Down),
            ],
        );
        let summary = summarize(&nodes, true);
        assert_eq!(
            (
                summary.nodes_total,
                summary.nodes_active,
                summary.nodes_degraded,
                summary.nodes_down
            ),
            (4, 2, 1, 1)
        );
        assert_eq!((summary.managers_total, summary.managers_reachable), (1, 1));
        assert_eq!(
            summary.quorum,
            QuorumState::Healthy,
            "workers down leave quorum"
        );
        assert_eq!(
            summary.cpu_percent,
            Some(20.0),
            "mean of the two online agents"
        );
        assert_eq!(summary.memory_used_bytes, 3 << 30);
        assert_eq!(summary.memory_total_bytes, 16 << 30);
        assert_eq!(summary.containers_running, 6);

        assert_eq!(
            summarize(&nodes, false).quorum,
            QuorumState::Degraded,
            "a manager that did not answer"
        );
        let mut unreachable = leader.clone();
        unreachable.manager_reachable = Some(false);
        let second = join_nodes(
            vec![agent("manager", NodeStatus::Online, Some("mgr"), 30.0, 1)],
            vec![leader, {
                let mut other = unreachable;
                other.id = "mgr2".to_owned();
                other.is_leader = false;
                other
            }],
        );
        let summary = summarize(&second, true);
        assert_eq!((summary.managers_total, summary.managers_reachable), (2, 1));
        assert_eq!(summary.quorum, QuorumState::Degraded);

        let empty = summarize(&[], false);
        assert_eq!(empty.cpu_percent, None);
        assert_eq!(empty.quorum, QuorumState::Degraded);
    }
}
