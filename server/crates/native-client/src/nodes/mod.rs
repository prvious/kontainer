//! Node membership and resource samples.
//!
//! Bodies are decoded strictly (unknown fields reject the response) and every
//! timestamp must be the controller's canonical RFC 3339 millisecond form.

mod list;
mod samples;
mod summary;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::client::parse_timestamp;
use crate::containers::NodeId;
use crate::error::NativeClientError;

/// Most points a series may carry.
pub const MAX_SAMPLE_POINTS: usize = 200;

/// The agent side, derived by the controller from the last check-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentStatus {
    /// Checked in within 15 seconds.
    Online,
    /// Silent for 15 to 60 seconds.
    Degraded,
    /// Silent for more than a minute.
    Offline,
    /// A Swarm member that never checked in.
    NoAgent,
}

/// Agent and Swarm state together: `active` only when both are fine, `down` when either says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeHealth {
    Active,
    Degraded,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeRole {
    Manager,
    Worker,
    /// The manager did not name the node.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeAvailability {
    Active,
    Pause,
    Drain,
}

/// What the manager last heard from the node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmNodeStatus {
    Ready,
    Down,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuorumState {
    Healthy,
    Degraded,
}

/// The newest sample a node reported.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeSample {
    pub sampled_at: DateTime<Utc>,
    pub cpu_percent: f64,
    pub memory_used_bytes: u64,
    pub disk_used_bytes: Option<u64>,
    pub containers_running: u32,
    pub containers_paused: u32,
    pub containers_exited: u32,
    pub net_rx_bytes_per_sec: Option<f64>,
    pub net_tx_bytes_per_sec: Option<f64>,
}

/// One node: membership joined to the manager's Swarm member. Agent fields are `None` for a
/// member without an agent, whose id is its Swarm node id.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeSummary {
    pub id: NodeId,
    pub hostname: String,
    pub agent_status: AgentStatus,
    pub health: NodeHealth,
    pub role: NodeRole,
    pub is_leader: bool,
    pub availability: Option<NodeAvailability>,
    pub swarm_status: SwarmNodeStatus,
    pub swarm_node_id: Option<String>,
    pub address: Option<String>,
    pub agent_version: Option<String>,
    pub docker_version: String,
    pub os: String,
    pub arch: String,
    pub cpu_cores: u32,
    pub memory_total_bytes: u64,
    pub agent_started_at: Option<DateTime<Utc>>,
    pub first_seen_at: Option<DateTime<Utc>>,
    pub last_check_in_at: Option<DateTime<Utc>>,
    pub latest_sample: Option<NodeSample>,
}

/// Node counts by health, manager reachability, quorum, and load over online nodes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClusterSummary {
    pub nodes_total: u32,
    pub nodes_active: u32,
    pub nodes_degraded: u32,
    pub nodes_down: u32,
    pub managers_total: u32,
    pub managers_reachable: u32,
    pub quorum: QuorumState,
    pub cpu_percent: Option<f64>,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub containers_running: u64,
    pub snapshot_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeList {
    pub items: Vec<NodeSummary>,
    pub snapshot_at: DateTime<Utc>,
}

/// One bucket; figures are `None` when no sample landed in it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplePoint {
    pub at: DateTime<Utc>,
    pub cpu_percent: Option<f64>,
    pub memory_used_bytes: Option<f64>,
    pub disk_used_bytes: Option<f64>,
    pub containers_running: Option<f64>,
    pub net_rx_bytes_per_sec: Option<f64>,
    pub net_tx_bytes_per_sec: Option<f64>,
    pub nodes: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SampleSeries {
    pub window: String,
    pub bucket: String,
    pub points: Vec<SamplePoint>,
    pub snapshot_at: DateTime<Utc>,
}

/// Windows the controller accepts.
pub const SAMPLE_WINDOWS: [&str; 5] = ["5m", "15m", "1h", "24h", "7d"];

/// Validate a window and bucket locally before any request: a known window, and a bucket of a
/// positive whole number of `s`, `m`, or `h`. The controller still checks divisibility.
pub(crate) fn validate_range(window: &str, bucket: &str) -> Result<(), NativeClientError> {
    let unit = bucket.chars().last().unwrap_or_default();
    let digits = &bucket[..bucket.len().saturating_sub(1)];
    let bucket_ok = matches!(unit, 's' | 'm' | 'h')
        && (1..=6).contains(&digits.len())
        && !digits.starts_with('0')
        && digits.bytes().all(|byte| byte.is_ascii_digit());
    if SAMPLE_WINDOWS.contains(&window) && bucket_ok {
        Ok(())
    } else {
        Err(NativeClientError::InvalidRequest)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NodeListBody {
    items: Vec<NodeBody>,
    snapshot_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NodeBody {
    id: String,
    hostname: String,
    agent_status: AgentStatus,
    health: NodeHealth,
    role: NodeRole,
    is_leader: bool,
    availability: Option<NodeAvailability>,
    swarm_status: SwarmNodeStatus,
    swarm_node_id: Option<String>,
    address: Option<String>,
    agent_version: Option<String>,
    docker_version: String,
    os: String,
    arch: String,
    cpu_cores: u32,
    memory_total_bytes: u64,
    agent_started_at: Option<String>,
    first_seen_at: Option<String>,
    last_check_in_at: Option<String>,
    latest_sample: Option<NodeSampleBody>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ClusterSummaryBody {
    nodes_total: u32,
    nodes_active: u32,
    nodes_degraded: u32,
    nodes_down: u32,
    managers_total: u32,
    managers_reachable: u32,
    quorum: QuorumState,
    cpu_percent: Option<f64>,
    memory_used_bytes: u64,
    memory_total_bytes: u64,
    containers_running: u64,
    snapshot_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NodeSampleBody {
    sampled_at: String,
    cpu_percent: f64,
    memory_used_bytes: u64,
    disk_used_bytes: Option<u64>,
    containers_running: u32,
    containers_paused: u32,
    containers_exited: u32,
    net_rx_bytes_per_sec: Option<f64>,
    net_tx_bytes_per_sec: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SampleSeriesBody {
    window: String,
    bucket: String,
    points: Vec<SamplePointBody>,
    snapshot_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SamplePointBody {
    at: String,
    cpu_percent: Option<f64>,
    memory_used_bytes: Option<f64>,
    disk_used_bytes: Option<f64>,
    containers_running: Option<f64>,
    net_rx_bytes_per_sec: Option<f64>,
    net_tx_bytes_per_sec: Option<f64>,
    nodes: u32,
}

fn finite(value: f64) -> Result<f64, NativeClientError> {
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(NativeClientError::InvalidResponse)
    }
}

fn optional_finite(value: Option<f64>) -> Result<Option<f64>, NativeClientError> {
    value.map(finite).transpose()
}

impl NodeListBody {
    pub(crate) fn into_list(self) -> Result<NodeList, NativeClientError> {
        let mut items = Vec::with_capacity(self.items.len());
        for node in self.items {
            let latest_sample = node
                .latest_sample
                .map(|sample| {
                    Ok::<_, NativeClientError>(NodeSample {
                        sampled_at: parse_timestamp(&sample.sampled_at)?,
                        cpu_percent: finite(sample.cpu_percent)?,
                        memory_used_bytes: sample.memory_used_bytes,
                        disk_used_bytes: sample.disk_used_bytes,
                        containers_running: sample.containers_running,
                        containers_paused: sample.containers_paused,
                        containers_exited: sample.containers_exited,
                        net_rx_bytes_per_sec: optional_finite(sample.net_rx_bytes_per_sec)?,
                        net_tx_bytes_per_sec: optional_finite(sample.net_tx_bytes_per_sec)?,
                    })
                })
                .transpose()?;
            let time = |value: Option<String>| value.as_deref().map(parse_timestamp).transpose();
            // An agent row carries every agent field; a Swarm-only row carries none.
            let has_agent = node.agent_status != AgentStatus::NoAgent;
            let agent_fields = [
                node.agent_version.is_some(),
                node.agent_started_at.is_some(),
                node.first_seen_at.is_some(),
                node.last_check_in_at.is_some(),
            ];
            if agent_fields.iter().any(|present| *present != has_agent)
                || (!has_agent && latest_sample.is_some())
            {
                return Err(NativeClientError::InvalidResponse);
            }
            items.push(NodeSummary {
                id: NodeId::parse(&node.id).map_err(|_| NativeClientError::InvalidResponse)?,
                hostname: node.hostname,
                agent_status: node.agent_status,
                health: node.health,
                role: node.role,
                is_leader: node.is_leader,
                availability: node.availability,
                swarm_status: node.swarm_status,
                swarm_node_id: node.swarm_node_id,
                address: node.address,
                agent_version: node.agent_version,
                docker_version: node.docker_version,
                os: node.os,
                arch: node.arch,
                cpu_cores: node.cpu_cores,
                memory_total_bytes: node.memory_total_bytes,
                agent_started_at: time(node.agent_started_at)?,
                first_seen_at: time(node.first_seen_at)?,
                last_check_in_at: time(node.last_check_in_at)?,
                latest_sample,
            });
        }
        Ok(NodeList {
            items,
            snapshot_at: parse_timestamp(&self.snapshot_at)?,
        })
    }
}

impl ClusterSummaryBody {
    pub(crate) fn into_summary(self) -> Result<ClusterSummary, NativeClientError> {
        let health_total = u64::from(self.nodes_active)
            + u64::from(self.nodes_degraded)
            + u64::from(self.nodes_down);
        if health_total != u64::from(self.nodes_total)
            || self.managers_reachable > self.managers_total
        {
            return Err(NativeClientError::InvalidResponse);
        }
        Ok(ClusterSummary {
            nodes_total: self.nodes_total,
            nodes_active: self.nodes_active,
            nodes_degraded: self.nodes_degraded,
            nodes_down: self.nodes_down,
            managers_total: self.managers_total,
            managers_reachable: self.managers_reachable,
            quorum: self.quorum,
            cpu_percent: optional_finite(self.cpu_percent)?,
            memory_used_bytes: self.memory_used_bytes,
            memory_total_bytes: self.memory_total_bytes,
            containers_running: self.containers_running,
            snapshot_at: parse_timestamp(&self.snapshot_at)?,
        })
    }
}

impl SampleSeriesBody {
    pub(crate) fn into_series(
        self,
        window: &str,
        bucket: &str,
    ) -> Result<SampleSeries, NativeClientError> {
        if self.window != window || self.bucket != bucket || self.points.len() > MAX_SAMPLE_POINTS {
            return Err(NativeClientError::InvalidResponse);
        }
        let mut points = Vec::with_capacity(self.points.len());
        for point in self.points {
            points.push(SamplePoint {
                at: parse_timestamp(&point.at)?,
                cpu_percent: optional_finite(point.cpu_percent)?,
                memory_used_bytes: optional_finite(point.memory_used_bytes)?,
                disk_used_bytes: optional_finite(point.disk_used_bytes)?,
                containers_running: optional_finite(point.containers_running)?,
                net_rx_bytes_per_sec: optional_finite(point.net_rx_bytes_per_sec)?,
                net_tx_bytes_per_sec: optional_finite(point.net_tx_bytes_per_sec)?,
                nodes: point.nodes,
            });
        }
        if points.windows(2).any(|pair| pair[0].at >= pair[1].at) {
            return Err(NativeClientError::InvalidResponse);
        }
        Ok(SampleSeries {
            window: self.window,
            bucket: self.bucket,
            points,
            snapshot_at: parse_timestamp(&self.snapshot_at)?,
        })
    }
}
