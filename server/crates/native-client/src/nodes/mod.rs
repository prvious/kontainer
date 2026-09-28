//! Node membership and resource samples.
//!
//! Bodies are decoded strictly (unknown fields reject the response) and every
//! timestamp must be the controller's canonical RFC 3339 millisecond form.

mod list;
mod samples;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::client::parse_timestamp;
use crate::containers::NodeId;
use crate::error::NativeClientError;

/// Most points a series may carry.
pub const MAX_SAMPLE_POINTS: usize = 200;

/// Membership status derived by the controller from the last check-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    /// Checked in within 15 seconds.
    Online,
    /// Silent for 15 to 60 seconds.
    Degraded,
    /// Silent for more than a minute.
    Offline,
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

/// One member node.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeSummary {
    pub id: NodeId,
    pub hostname: String,
    pub status: NodeStatus,
    pub swarm_node_id: Option<String>,
    pub role: Option<String>,
    pub availability: Option<String>,
    pub agent_version: String,
    pub docker_version: String,
    pub os: String,
    pub arch: String,
    pub cpu_cores: u32,
    pub memory_total_bytes: u64,
    pub agent_started_at: DateTime<Utc>,
    pub first_seen_at: DateTime<Utc>,
    pub last_check_in_at: DateTime<Utc>,
    pub latest_sample: Option<NodeSample>,
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
    status: NodeStatus,
    swarm_node_id: Option<String>,
    role: Option<String>,
    availability: Option<String>,
    agent_version: String,
    docker_version: String,
    os: String,
    arch: String,
    cpu_cores: u32,
    memory_total_bytes: u64,
    agent_started_at: String,
    first_seen_at: String,
    last_check_in_at: String,
    latest_sample: Option<NodeSampleBody>,
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
            items.push(NodeSummary {
                id: NodeId::parse(&node.id).map_err(|_| NativeClientError::InvalidResponse)?,
                hostname: node.hostname,
                status: node.status,
                swarm_node_id: node.swarm_node_id,
                role: node.role,
                availability: node.availability,
                agent_version: node.agent_version,
                docker_version: node.docker_version,
                os: node.os,
                arch: node.arch,
                cpu_cores: node.cpu_cores,
                memory_total_bytes: node.memory_total_bytes,
                agent_started_at: parse_timestamp(&node.agent_started_at)?,
                first_seen_at: parse_timestamp(&node.first_seen_at)?,
                last_check_in_at: parse_timestamp(&node.last_check_in_at)?,
                latest_sample,
            });
        }
        Ok(NodeList {
            items,
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
