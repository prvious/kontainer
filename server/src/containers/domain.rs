//! Containers across the cluster, and the payloads their operations carry.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    operations::{bulk::BulkTarget, domain::OperationKind},
    rpc::{AgentContainerAction, AgentContainerState},
};

/// The merged list refuses beyond this; callers filter by node instead.
pub const MAX_CONTAINERS: usize = 2_000;
pub const MAX_STOP_TIMEOUT_SECONDS: u32 = 600;

/// One container on one node, with the node's display name.
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerItem {
    pub node_id: String,
    pub node_name: String,
    pub container_id: String,
    pub name: String,
    pub image: String,
    pub state: AgentContainerState,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub ports: Vec<String>,
    pub service_name: Option<String>,
    pub stack_namespace: Option<String>,
    pub task_id: Option<String>,
    pub cpu_percent: Option<f64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_limit_bytes: Option<u64>,
    pub networks: Vec<String>,
}

/// The operation kind for one lifecycle action.
#[must_use]
pub const fn action_kind(action: AgentContainerAction) -> OperationKind {
    match action {
        AgentContainerAction::Start => OperationKind::ContainerStart,
        AgentContainerAction::Stop => OperationKind::ContainerStop,
        AgentContainerAction::Restart => OperationKind::ContainerRestart,
        AgentContainerAction::Pause => OperationKind::ContainerPause,
        AgentContainerAction::Unpause => OperationKind::ContainerUnpause,
        AgentContainerAction::Remove => OperationKind::ContainerRemove,
    }
}

/// The stored request of a `container.*` operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContainerActionPayload {
    pub node_id: String,
    pub container_id: String,
    pub action: String,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub timeout_seconds: Option<u32>,
}

/// The stored request of a `containers.bulk` operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BulkActionPayload {
    pub action: String,
    pub targets: Vec<BulkTarget>,
    #[serde(default)]
    pub force: bool,
}
