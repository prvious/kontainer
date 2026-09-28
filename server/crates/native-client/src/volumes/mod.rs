//! Node-scoped volumes: the list (with the create dialog's conflict query), create, and remove.
//!
//! Volumes are node-local, so a row is a node and a name. Create and remove are admitted
//! operations (see [`crate::operations`]).

mod calls;

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::client::parse_timestamp;
use crate::containers::{NodeId, NodeOutcome, NodeOutcomeBody};
use crate::error::NativeClientError;

/// The most rows one list may carry.
pub const MAX_VOLUMES: usize = 4_000;
/// Labels one volume may carry.
pub const MAX_VOLUME_LABELS: usize = 64;
/// The only driver the controller offers in this phase.
pub const LOCAL_VOLUME_DRIVER: &str = "local";

const MAX_LABEL_LEN: usize = 256;
const MAX_USERS: usize = 16;

/// A volume name: `[a-zA-Z0-9][a-zA-Z0-9_.-]+`, 2–255 characters (Docker's rule).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VolumeName(String);

impl VolumeName {
    pub fn parse(value: &str) -> Result<Self, NativeClientError> {
        let bytes = value.as_bytes();
        let valid = (2..=255).contains(&bytes.len())
            && bytes[0].is_ascii_alphanumeric()
            && bytes[1..]
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'));
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

/// One volume on one node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeItem {
    pub node_id: NodeId,
    pub name: String,
    pub driver: String,
    pub mount_point: String,
    pub created_at: Option<DateTime<Utc>>,
    pub labels: BTreeMap<String, String>,
    /// Containers on the node (any state) that mount it.
    pub used_by: u32,
    /// Up to 16 of their names.
    pub users: Vec<String>,
    /// `None` until the node's first disk sample, or when the driver can't say.
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeList {
    pub items: Vec<VolumeItem>,
    pub nodes: Vec<NodeOutcome>,
    pub snapshot_at: DateTime<Utc>,
}

/// Narrow the list to one node, one name, or both.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VolumeQuery {
    pub node: Option<NodeId>,
    pub name: Option<VolumeName>,
}

/// What a create asks for; the driver is always `local` for now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeSpec {
    pub node_id: NodeId,
    pub name: VolumeName,
    pub labels: BTreeMap<String, String>,
}

pub(crate) fn valid_labels(labels: &BTreeMap<String, String>) -> bool {
    labels.len() <= MAX_VOLUME_LABELS
        && labels.iter().all(|(key, value)| {
            !key.is_empty()
                && key.len() <= MAX_LABEL_LEN
                && value.len() <= MAX_LABEL_LEN
                && !key
                    .chars()
                    .any(|character| character.is_control() || character == '=')
                && !value.chars().any(char::is_control)
        })
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct VolumeListBody {
    items: Vec<VolumeItemBody>,
    nodes: Vec<NodeOutcomeBody>,
    snapshot_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct VolumeItemBody {
    node_id: String,
    name: String,
    driver: String,
    mount_point: String,
    created_at: Option<String>,
    labels: BTreeMap<String, String>,
    used_by: u32,
    users: Vec<String>,
    size_bytes: Option<u64>,
}

impl VolumeListBody {
    pub(crate) fn into_list(self) -> Result<VolumeList, NativeClientError> {
        if self.items.len() > MAX_VOLUMES {
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
                if !members.contains(&item.node_id)
                    || VolumeName::parse(&item.name).is_err()
                    || item.driver.len() > 512
                    || item.mount_point.len() > 4096
                    || !valid_labels(&item.labels)
                    || item.users.len() > MAX_USERS
                {
                    return Err(NativeClientError::InvalidResponse);
                }
                Ok(VolumeItem {
                    node_id: NodeId::parse(&item.node_id)
                        .map_err(|_| NativeClientError::InvalidResponse)?,
                    name: item.name,
                    driver: item.driver,
                    mount_point: item.mount_point,
                    created_at: item
                        .created_at
                        .as_deref()
                        .map(parse_timestamp)
                        .transpose()?,
                    labels: item.labels,
                    used_by: item.used_by,
                    users: item.users,
                    size_bytes: item.size_bytes,
                })
            })
            .collect::<Result<Vec<_>, NativeClientError>>()?;
        Ok(VolumeList {
            items,
            nodes,
            snapshot_at: parse_timestamp(&self.snapshot_at)?,
        })
    }
}
