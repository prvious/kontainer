//! Networks across the cluster: the merged list, one network's attached containers, create, and
//! remove. Swarm-scope networks are one row for the cluster; local ones one row per node.

mod calls;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::client::{parse_timestamp, valid_display_name};
use crate::containers::{NodeId, NodeOutcome, NodeOutcomeBody};
use crate::error::NativeClientError;

/// The most rows one list may carry.
pub const MAX_NETWORKS: usize = 4_000;
const MAX_ATTACHED: usize = 4_096;

/// A Docker network id: 25 (swarm) or 64 (local) lowercase alphanumerics.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NetworkId(String);

impl NetworkId {
    pub fn parse(value: &str) -> Result<Self, NativeClientError> {
        let valid = matches!(value.len(), 25 | 64)
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase());
        if valid {
            Ok(Self(value.to_owned()))
        } else {
            Err(NativeClientError::InvalidRequest)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether a network spans the swarm or lives on one node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkScope {
    Swarm,
    Local,
}

/// Whether the swarm-scope rows came from the manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagerStatus {
    Ok,
    NotAManager,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkItem {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: NetworkScope,
    pub subnets: Vec<String>,
    /// Running containers attached, summed across nodes.
    pub containers: u32,
    pub created_at: Option<DateTime<Utc>>,
    pub attachable: bool,
    pub internal: bool,
    pub builtin: bool,
    /// Local: its node. Swarm: the nodes with endpoints on it.
    pub node_ids: Vec<NodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkList {
    pub items: Vec<NetworkItem>,
    pub nodes: Vec<NodeOutcome>,
    pub manager: ManagerStatus,
    pub snapshot_at: DateTime<Utc>,
}

/// One container attached to a network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachedContainer {
    pub node_id: NodeId,
    pub container_id: String,
    pub name: String,
}

/// Which driver a create asks for, and where it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkPlacement {
    /// Swarm scope, on the manager.
    Overlay { attachable: bool },
    /// Local, on one node.
    Bridge { node_id: NodeId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkSpec {
    pub name: String,
    pub placement: NetworkPlacement,
    pub subnet: Option<String>,
}

pub(crate) fn valid_network_name(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=128).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NetworkListBody {
    items: Vec<NetworkItemBody>,
    nodes: Vec<NodeOutcomeBody>,
    manager: ManagerStatus,
    snapshot_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NetworkItemBody {
    id: String,
    name: String,
    driver: String,
    scope: NetworkScope,
    subnets: Vec<String>,
    containers: u32,
    created_at: Option<String>,
    attachable: bool,
    internal: bool,
    builtin: bool,
    node_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NetworkContainersBody {
    containers: Vec<AttachedContainerBody>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AttachedContainerBody {
    node_id: String,
    container_id: String,
    name: String,
}

impl NetworkListBody {
    pub(crate) fn into_list(self) -> Result<NetworkList, NativeClientError> {
        if self.items.len() > MAX_NETWORKS {
            return Err(NativeClientError::InvalidResponse);
        }
        let nodes = self
            .nodes
            .into_iter()
            .map(NodeOutcomeBody::into_outcome)
            .collect::<Result<Vec<_>, NativeClientError>>()?;
        let members: std::collections::HashSet<_> = nodes
            .iter()
            .map(|node| node.node_id.as_str().to_owned())
            .collect();
        let items = self
            .items
            .into_iter()
            .map(|item| {
                let nodes_ok = match item.scope {
                    NetworkScope::Local => item.node_ids.len() == 1,
                    NetworkScope::Swarm => true,
                };
                if NetworkId::parse(&item.id).is_err()
                    || !valid_display_name(&item.name)
                    || item.driver.len() > 512
                    || item.subnets.len() > 16
                    || !nodes_ok
                    || !item
                        .node_ids
                        .iter()
                        .all(|node_id| members.contains(node_id))
                {
                    return Err(NativeClientError::InvalidResponse);
                }
                Ok(NetworkItem {
                    id: item.id,
                    name: item.name,
                    driver: item.driver,
                    scope: item.scope,
                    subnets: item.subnets,
                    containers: item.containers,
                    created_at: item
                        .created_at
                        .as_deref()
                        .map(parse_timestamp)
                        .transpose()?,
                    attachable: item.attachable,
                    internal: item.internal,
                    builtin: item.builtin,
                    node_ids: item
                        .node_ids
                        .iter()
                        .map(|node_id| {
                            NodeId::parse(node_id).map_err(|_| NativeClientError::InvalidResponse)
                        })
                        .collect::<Result<_, _>>()?,
                })
            })
            .collect::<Result<Vec<_>, NativeClientError>>()?;
        Ok(NetworkList {
            items,
            nodes,
            manager: self.manager,
            snapshot_at: parse_timestamp(&self.snapshot_at)?,
        })
    }
}

impl NetworkContainersBody {
    pub(crate) fn into_containers(self) -> Result<Vec<AttachedContainer>, NativeClientError> {
        if self.containers.len() > MAX_ATTACHED {
            return Err(NativeClientError::InvalidResponse);
        }
        self.containers
            .into_iter()
            .map(|container| {
                if !crate::containers::valid_container_id(&container.container_id)
                    || container.name.len() > 512
                {
                    return Err(NativeClientError::InvalidResponse);
                }
                Ok(AttachedContainer {
                    node_id: NodeId::parse(&container.node_id)
                        .map_err(|_| NativeClientError::InvalidResponse)?,
                    container_id: container.container_id,
                    name: container.name,
                })
            })
            .collect()
    }
}
