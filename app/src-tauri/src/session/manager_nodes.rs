//! Node membership, samples, and controller settings, as credential-free IPC views.
//!
//! Mirrors `app/src/types/nodes.ts`; the shared fixture `app/tests/fixtures/nodes-ipc.json`
//! keeps both sides in sync.

use kontainer_native_client::{
    AgentStatus, ClusterSummary, ControllerSettings, ControllerStatus, NodeAvailability,
    NodeHealth, NodeId, NodeList, NodeRole, NodeSample, NodeSummary, QuorumState, SamplePoint,
    SampleSeries, SwarmNodeStatus,
};
use serde::Serialize;
use uuid::Uuid;

use super::{inventory::InventoryError, SessionManager};

/// The controller's canonical RFC 3339 millisecond form.
macro_rules! timestamp {
    ($value:expr) => {
        $value.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
    };
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NodeListView {
    items: Vec<NodeView>,
    snapshot_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeView {
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
    latest_sample: Option<NodeSampleView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClusterSummaryView {
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeSampleView {
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SampleSeriesView {
    window: String,
    bucket: String,
    points: Vec<SamplePointView>,
    snapshot_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SamplePointView {
    at: String,
    cpu_percent: Option<f64>,
    memory_used_bytes: Option<f64>,
    disk_used_bytes: Option<f64>,
    containers_running: Option<f64>,
    net_rx_bytes_per_sec: Option<f64>,
    net_tx_bytes_per_sec: Option<f64>,
    nodes: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ControllerSettingsView {
    metrics_retention_days: u32,
}

/// The controller's version, log filter, and start time, for the sidebar and Settings.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ControllerStatusView {
    version: String,
    log_level: String,
    started_at: String,
}

impl From<ControllerStatus> for ControllerStatusView {
    fn from(status: ControllerStatus) -> Self {
        Self {
            version: status.version,
            log_level: status.log_level,
            started_at: timestamp!(status.started_at),
        }
    }
}

impl From<NodeSample> for NodeSampleView {
    fn from(sample: NodeSample) -> Self {
        Self {
            sampled_at: timestamp!(sample.sampled_at),
            cpu_percent: sample.cpu_percent,
            memory_used_bytes: sample.memory_used_bytes,
            disk_used_bytes: sample.disk_used_bytes,
            containers_running: sample.containers_running,
            containers_paused: sample.containers_paused,
            containers_exited: sample.containers_exited,
            net_rx_bytes_per_sec: sample.net_rx_bytes_per_sec,
            net_tx_bytes_per_sec: sample.net_tx_bytes_per_sec,
        }
    }
}

impl From<NodeSummary> for NodeView {
    fn from(node: NodeSummary) -> Self {
        Self {
            id: node.id.to_string(),
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
            agent_started_at: node.agent_started_at.map(|at| timestamp!(at)),
            first_seen_at: node.first_seen_at.map(|at| timestamp!(at)),
            last_check_in_at: node.last_check_in_at.map(|at| timestamp!(at)),
            latest_sample: node.latest_sample.map(NodeSampleView::from),
        }
    }
}

impl From<NodeList> for NodeListView {
    fn from(list: NodeList) -> Self {
        Self {
            items: list.items.into_iter().map(NodeView::from).collect(),
            snapshot_at: timestamp!(list.snapshot_at),
        }
    }
}

impl From<ClusterSummary> for ClusterSummaryView {
    fn from(summary: ClusterSummary) -> Self {
        Self {
            nodes_total: summary.nodes_total,
            nodes_active: summary.nodes_active,
            nodes_degraded: summary.nodes_degraded,
            nodes_down: summary.nodes_down,
            managers_total: summary.managers_total,
            managers_reachable: summary.managers_reachable,
            quorum: summary.quorum,
            cpu_percent: summary.cpu_percent,
            memory_used_bytes: summary.memory_used_bytes,
            memory_total_bytes: summary.memory_total_bytes,
            containers_running: summary.containers_running,
            snapshot_at: timestamp!(summary.snapshot_at),
        }
    }
}

impl From<SamplePoint> for SamplePointView {
    fn from(point: SamplePoint) -> Self {
        Self {
            at: timestamp!(point.at),
            cpu_percent: point.cpu_percent,
            memory_used_bytes: point.memory_used_bytes,
            disk_used_bytes: point.disk_used_bytes,
            containers_running: point.containers_running,
            net_rx_bytes_per_sec: point.net_rx_bytes_per_sec,
            net_tx_bytes_per_sec: point.net_tx_bytes_per_sec,
            nodes: point.nodes,
        }
    }
}

impl From<SampleSeries> for SampleSeriesView {
    fn from(series: SampleSeries) -> Self {
        Self {
            window: series.window,
            bucket: series.bucket,
            points: series
                .points
                .into_iter()
                .map(SamplePointView::from)
                .collect(),
            snapshot_at: timestamp!(series.snapshot_at),
        }
    }
}

impl From<ControllerSettings> for ControllerSettingsView {
    fn from(settings: ControllerSettings) -> Self {
        Self {
            metrics_retention_days: settings.metrics_retention_days,
        }
    }
}

impl SessionManager {
    pub(crate) async fn nodes(&self, profile_id: Uuid) -> Result<NodeListView, InventoryError> {
        let list = self
            .authenticated(profile_id, &(), |controller, access, cancellation, ()| {
                controller.list_nodes(access, cancellation)
            })
            .await?;
        Ok(list.into())
    }

    pub(crate) async fn cluster_summary(
        &self,
        profile_id: Uuid,
    ) -> Result<ClusterSummaryView, InventoryError> {
        let summary = self
            .authenticated(profile_id, &(), |controller, access, cancellation, ()| {
                controller.cluster_summary(access, cancellation)
            })
            .await?;
        Ok(summary.into())
    }

    /// `node_id` `None` reads the cluster series.
    pub(crate) async fn samples(
        &self,
        profile_id: Uuid,
        node_id: Option<&str>,
        window: &str,
        bucket: &str,
    ) -> Result<SampleSeriesView, InventoryError> {
        let node = node_id.map(NodeId::parse).transpose()?;
        let series = self
            .authenticated(
                profile_id,
                &(node, window, bucket),
                |controller, access, cancellation, (node, window, bucket)| {
                    controller.samples(access, node.as_ref(), window, bucket, cancellation)
                },
            )
            .await?;
        Ok(series.into())
    }

    pub(crate) async fn controller_status(
        &self,
        profile_id: Uuid,
    ) -> Result<ControllerStatusView, InventoryError> {
        let status = self
            .authenticated(profile_id, &(), |controller, access, cancellation, ()| {
                controller.get_status(access, cancellation)
            })
            .await?;
        Ok(status.into())
    }

    pub(crate) async fn controller_settings(
        &self,
        profile_id: Uuid,
    ) -> Result<ControllerSettingsView, InventoryError> {
        let settings = self
            .authenticated(profile_id, &(), |controller, access, cancellation, ()| {
                controller.get_settings(access, cancellation)
            })
            .await?;
        Ok(settings.into())
    }

    pub(crate) async fn update_controller_settings(
        &self,
        profile_id: Uuid,
        metrics_retention_days: u32,
    ) -> Result<ControllerSettingsView, InventoryError> {
        let settings = self
            .authenticated(
                profile_id,
                &metrics_retention_days,
                |controller, access, cancellation, days| {
                    controller.update_settings(access, *days, cancellation)
                },
            )
            .await?;
        Ok(settings.into())
    }
}
