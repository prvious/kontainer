//! Containers across the cluster and their lifecycle actions.
//!
//! The list is one bounded response with an outcome per member node; the client rejects a body
//! with unknown fields, non-canonical ids or times, or more items than the controller's cap.
//! Actions are admitted operations (see [`crate::operations`]).

mod actions;
mod list;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::client::parse_timestamp;
use crate::credentials::is_canonical_hex;
use crate::error::NativeClientError;

/// The controller refuses to list more than this at once.
pub const MAX_CONTAINERS: usize = 2_000;
/// Stop and restart grace the controller accepts, in seconds.
pub const MAX_STOP_TIMEOUT_SECONDS: u32 = 600;
/// Targets one bulk action may name.
pub const MAX_BULK_TARGETS: usize = 200;

const CONTAINER_ID_HEX_LEN: usize = 64;
const MAX_NODE_ID_LEN: usize = 128;
const MAX_PORTS: usize = 64;

/// A canonical controller node identifier.
///
/// Mirrors the controller's identity-segment rules: 1-128 ASCII letters,
/// digits, dots, underscores, or hyphens, excluding `.` and `..`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(String);

impl NodeId {
    /// Validate a node identifier.
    pub fn parse(value: &str) -> Result<Self, NativeClientError> {
        if !valid_node_id(value) {
            return Err(NativeClientError::InvalidNodeId);
        }
        Ok(Self(value.to_owned()))
    }

    /// The canonical identifier as the controller knows it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Typed lifecycle state of one container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerState {
    /// The daemon reported a state the controller does not classify.
    Unknown,
    Created,
    Running,
    Paused,
    Restarting,
    Removing,
    Exited,
    Dead,
    Stopping,
}

/// One container on one node. No environment, command, mounts, or labels beyond the three
/// Swarm ones ever reach this type.
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerItem {
    pub node_id: NodeId,
    pub node_name: String,
    pub container_id: String,
    pub name: String,
    pub image: String,
    pub state: ContainerState,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    /// Published ports, `host:container/proto`.
    pub ports: Vec<String>,
    pub service_name: Option<String>,
    pub stack_namespace: Option<String>,
    pub task_id: Option<String>,
    pub cpu_percent: Option<f64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_limit_bytes: Option<u64>,
}

/// How one member node answered a fan-out read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeOutcomeStatus {
    Ok,
    Timeout,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeOutcome {
    pub node_id: NodeId,
    pub status: NodeOutcomeStatus,
    /// A stable code when `status` is `error`.
    pub message: Option<String>,
}

/// The merged list: containers from every answering node, one outcome per member.
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerList {
    pub items: Vec<ContainerItem>,
    pub nodes: Vec<NodeOutcome>,
    pub snapshot_at: DateTime<Utc>,
}

/// Lifecycle actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContainerAction {
    Start,
    Stop,
    Restart,
    Pause,
    Unpause,
    Remove,
}

impl ContainerAction {
    pub const ALL: [Self; 6] = [
        Self::Start,
        Self::Stop,
        Self::Restart,
        Self::Pause,
        Self::Unpause,
        Self::Remove,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Pause => "pause",
            Self::Unpause => "unpause",
            Self::Remove => "remove",
        }
    }

    pub fn parse(value: &str) -> Result<Self, NativeClientError> {
        Self::ALL
            .into_iter()
            .find(|action| action.as_str() == value)
            .ok_or(NativeClientError::InvalidRequest)
    }
}

/// Options for one action; the controller ignores `force` except on remove and the timeout
/// except on stop and restart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContainerActionOptions {
    pub force: bool,
    pub timeout_seconds: Option<u32>,
}

/// One container on one node: what an action names.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContainerTarget {
    pub node_id: NodeId,
    pub container_id: String,
}

fn valid_node_id(value: &str) -> bool {
    !value.is_empty()
        && !matches!(value, "." | "..")
        && value.len() <= MAX_NODE_ID_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

pub(crate) fn valid_container_id(value: &str) -> bool {
    is_canonical_hex(value, CONTAINER_ID_HEX_LEN)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ContainerListBody {
    items: Vec<ContainerItemBody>,
    nodes: Vec<NodeOutcomeBody>,
    snapshot_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ContainerItemBody {
    node_id: String,
    node_name: String,
    container_id: String,
    name: String,
    image: String,
    state: ContainerState,
    status: String,
    created_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    ports: Vec<String>,
    service_name: Option<String>,
    stack_namespace: Option<String>,
    task_id: Option<String>,
    cpu_percent: Option<f64>,
    memory_used_bytes: Option<u64>,
    memory_limit_bytes: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NodeOutcomeBody {
    node_id: String,
    status: NodeOutcomeStatus,
    #[serde(default)]
    message: Option<String>,
}

fn optional_time(value: Option<&str>) -> Result<Option<DateTime<Utc>>, NativeClientError> {
    value.map(parse_timestamp).transpose()
}

impl ContainerListBody {
    pub(crate) fn into_list(self) -> Result<ContainerList, NativeClientError> {
        if self.items.len() > MAX_CONTAINERS {
            return Err(NativeClientError::InvalidResponse);
        }
        let nodes = self
            .nodes
            .into_iter()
            .map(|node| {
                Ok(NodeOutcome {
                    node_id: NodeId::parse(&node.node_id)
                        .map_err(|_| NativeClientError::InvalidResponse)?,
                    status: node.status,
                    message: node.message,
                })
            })
            .collect::<Result<Vec<_>, NativeClientError>>()?;
        // Items only come from nodes that answered.
        let answered: std::collections::HashSet<_> = nodes
            .iter()
            .filter(|node| node.status == NodeOutcomeStatus::Ok)
            .map(|node| node.node_id.as_str().to_owned())
            .collect();
        let mut items = Vec::with_capacity(self.items.len());
        for item in self.items {
            let figure_ok = item
                .cpu_percent
                .is_none_or(|cpu| cpu.is_finite() && cpu >= 0.0);
            if !valid_container_id(&item.container_id)
                || !answered.contains(&item.node_id)
                || item.ports.len() > MAX_PORTS
                || !figure_ok
            {
                return Err(NativeClientError::InvalidResponse);
            }
            items.push(ContainerItem {
                node_id: NodeId::parse(&item.node_id)
                    .map_err(|_| NativeClientError::InvalidResponse)?,
                node_name: item.node_name,
                container_id: item.container_id,
                name: item.name,
                image: item.image,
                state: item.state,
                status: item.status,
                created_at: parse_timestamp(&item.created_at)?,
                started_at: optional_time(item.started_at.as_deref())?,
                finished_at: optional_time(item.finished_at.as_deref())?,
                ports: item.ports,
                service_name: item.service_name,
                stack_namespace: item.stack_namespace,
                task_id: item.task_id,
                cpu_percent: item.cpu_percent,
                memory_used_bytes: item.memory_used_bytes,
                memory_limit_bytes: item.memory_limit_bytes,
            });
        }
        Ok(ContainerList {
            items,
            nodes,
            snapshot_at: parse_timestamp(&self.snapshot_at)?,
        })
    }
}
