//! Images across the cluster: the merged list, removal, and pulls.
//!
//! The list merges one image id across nodes into one row with the nodes that hold it. Removal
//! and pulls are admitted operations (see [`crate::operations`]); a pull's per-node progress is
//! the operation's `progress` detail.

mod list;
mod mutations;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::client::parse_timestamp;
use crate::containers::{NodeId, NodeOutcome, NodeOutcomeBody};
use crate::credentials::is_canonical_hex;
use crate::error::NativeClientError;

/// The most image rows one list may carry.
pub const MAX_IMAGES: usize = 4_000;
/// Nodes one removal or pull may name.
pub const MAX_IMAGE_TARGET_NODES: usize = 64;
/// Longest image reference the controller accepts.
pub const MAX_IMAGE_REFERENCE_LEN: usize = 512;

const IMAGE_ID_HEX_LEN: usize = 64;
const MAX_TAGS: usize = 64;

/// One image, merged by id across the nodes that answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageItem {
    /// `sha256:<64 hex>`.
    pub image_id: String,
    pub repo_tags: Vec<String>,
    pub repo_digests: Vec<String>,
    /// Counted once, however many nodes hold it.
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
    /// Containers using it, summed across nodes.
    pub used_by: u32,
    pub dangling: bool,
    pub node_ids: Vec<NodeId>,
}

/// The merged list with one outcome per member node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageList {
    pub items: Vec<ImageItem>,
    pub nodes: Vec<NodeOutcome>,
    pub snapshot_at: DateTime<Utc>,
}

/// Which nodes a removal targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageNodes {
    /// Every member node that holds the image.
    All,
    /// 1–64 distinct nodes.
    Only(Vec<NodeId>),
}

pub(crate) fn valid_image_id(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| is_canonical_hex(hex, IMAGE_ID_HEX_LEN))
}

pub(crate) fn valid_target_nodes(node_ids: &[NodeId]) -> bool {
    let distinct: std::collections::HashSet<_> = node_ids.iter().collect();
    !node_ids.is_empty()
        && node_ids.len() <= MAX_IMAGE_TARGET_NODES
        && distinct.len() == node_ids.len()
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ImageListBody {
    items: Vec<ImageItemBody>,
    nodes: Vec<NodeOutcomeBody>,
    snapshot_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ImageItemBody {
    image_id: String,
    repo_tags: Vec<String>,
    repo_digests: Vec<String>,
    size_bytes: u64,
    created_at: String,
    used_by: u32,
    dangling: bool,
    node_ids: Vec<String>,
}

impl ImageListBody {
    pub(crate) fn into_list(self) -> Result<ImageList, NativeClientError> {
        if self.items.len() > MAX_IMAGES {
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
                if !valid_image_id(&item.image_id)
                    || item.repo_tags.len() > MAX_TAGS
                    || item.repo_digests.len() > MAX_TAGS
                    || item.node_ids.is_empty()
                    || !item
                        .node_ids
                        .iter()
                        .all(|node_id| members.contains(node_id))
                {
                    return Err(NativeClientError::InvalidResponse);
                }
                Ok(ImageItem {
                    image_id: item.image_id,
                    repo_tags: item.repo_tags,
                    repo_digests: item.repo_digests,
                    size_bytes: item.size_bytes,
                    created_at: parse_timestamp(&item.created_at)?,
                    used_by: item.used_by,
                    dangling: item.dangling,
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
        Ok(ImageList {
            items,
            nodes,
            snapshot_at: parse_timestamp(&self.snapshot_at)?,
        })
    }
}
