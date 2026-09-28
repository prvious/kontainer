//! Node membership in the controller's own terms, distinct from protobuf and persistence.

use chrono::{DateTime, Utc};

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

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use super::{CheckIn, CheckInRejection, NodeStatus, SampleReading, valid_host_port, validate};

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
}
