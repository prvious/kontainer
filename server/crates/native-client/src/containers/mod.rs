//! Bounded node-container collection.
//!
//! The collection is read-only, node-scoped, and bounded on both ends: the
//! client validates the page request before any network access and rejects a
//! response that exceeds the hard page size, carries a malformed cursor, or
//! claims container provenance from a different node.

mod list;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::client::NativeClient;
use crate::client::parse_timestamp;
use crate::credentials::is_canonical_hex;
use crate::error::NativeClientError;

/// Default page size when the caller requests no explicit limit.
pub const DEFAULT_CONTAINER_LIMIT: u32 = 50;
/// Hard maximum page size the controller documents.
pub const MAX_CONTAINER_LIMIT: u32 = 100;
/// Maximum length of the opaque pagination cursor.
pub const MAX_CONTAINER_CURSOR_LEN: usize = 67;

const CURSOR_PREFIX: &str = "v1:";
const CONTAINER_ID_HEX_LEN: usize = 64;
const MAX_NODE_ID_LEN: usize = 128;

/// A canonical controller node identifier.
///
/// Mirrors the controller's identity-segment rules: 1-128 ASCII letters,
/// digits, dots, underscores, or hyphens, excluding `.` and `..`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeId(String);

impl NodeId {
    /// Validate a configured node identifier.
    pub fn parse(value: &str) -> Result<Self, NativeClientError> {
        if !valid_node_id(value) {
            return Err(NativeClientError::InvalidNodeId);
        }
        Ok(Self(value.to_owned()))
    }

    /// The canonical identifier as configured on the controller.
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

/// One page request against a node's container collection.
///
/// [`Default`] requests the first page at the documented default size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerPageRequest {
    /// Requested page size, `1..=100`.
    pub limit: u32,
    /// Opaque cursor from a previous page; `None` starts at the first page.
    pub cursor: Option<String>,
}

impl Default for ContainerPageRequest {
    fn default() -> Self {
        Self {
            limit: DEFAULT_CONTAINER_LIMIT,
            cursor: None,
        }
    }
}

/// Typed lifecycle state of one container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerState {
    /// The daemon reported a state the controller does not classify.
    Unknown,
    /// Created but never started.
    Created,
    /// Running.
    Running,
    /// Paused.
    Paused,
    /// Restarting.
    Restarting,
    /// Being removed.
    Removing,
    /// Exited.
    Exited,
    /// Dead.
    Dead,
    /// Shutting down.
    Stopping,
}

/// One container summary. The controller never exposes environment,
/// commands, labels, mounts, ports, networking, or raw inspect data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerItem {
    /// Full lowercase-hex daemon container ID.
    pub container_id: String,
    /// Canonical display name without Docker linkage.
    pub display_name: String,
    /// Image reference as reported by the daemon.
    pub image_reference: String,
    /// Typed lifecycle state.
    pub state: ContainerState,
    /// Bounded daemon status text.
    pub status: String,
    /// Creation time reported by the controller.
    pub created_at: DateTime<Utc>,
    /// Serving node ID from agent configuration, never request input.
    pub node_id: String,
}

/// One bounded page preserving the controller's deterministic order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerPage {
    /// At most [`MAX_CONTAINER_LIMIT`] items.
    pub items: Vec<ContainerItem>,
    /// Opaque cursor for the next page, or `None` when this page is last.
    pub next_cursor: Option<String>,
}

impl ContainerPageRequest {
    /// Validate the page request locally; no network access follows an error.
    pub(crate) fn validated(&self) -> Result<(u32, Option<&str>), NativeClientError> {
        if !(1..=MAX_CONTAINER_LIMIT).contains(&self.limit) {
            return Err(NativeClientError::InvalidPagination);
        }
        match self.cursor.as_deref() {
            None | Some("") => Ok((self.limit, None)),
            Some(cursor) if valid_cursor_token(cursor) => Ok((self.limit, Some(cursor))),
            Some(_) => Err(NativeClientError::InvalidPagination),
        }
    }
}

impl NativeClient {
    /// Node collection endpoint; the node segment is validated and encoded by
    /// the URL parser, never interpolated blindly.
    pub(crate) fn containers_endpoint(&self, node_id: &NodeId) -> Result<Url, NativeClientError> {
        let mut url = self.endpoint("/api/v1/nodes")?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| NativeClientError::InvalidConfiguration)?;
            segments.push(node_id.as_str());
            segments.push("containers");
        }
        Ok(url)
    }
}

fn valid_node_id(value: &str) -> bool {
    !value.is_empty()
        && !matches!(value, "." | "..")
        && value.len() <= MAX_NODE_ID_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

pub(crate) fn valid_cursor_token(token: &str) -> bool {
    token.len() <= MAX_CONTAINER_CURSOR_LEN
        && token
            .strip_prefix(CURSOR_PREFIX)
            .is_some_and(|id| is_canonical_hex(id, CONTAINER_ID_HEX_LEN))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ContainerPageBody {
    items: Vec<ContainerItemBody>,
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ContainerItemBody {
    container_id: String,
    display_name: String,
    image_reference: String,
    state: ContainerState,
    status: String,
    created_at: String,
    node_id: String,
}

impl ContainerPageBody {
    pub(crate) fn into_page(self, requested: &NodeId) -> Result<ContainerPage, NativeClientError> {
        if self.items.len() > MAX_CONTAINER_LIMIT as usize {
            return Err(NativeClientError::InvalidResponse);
        }
        let next_cursor = match self.next_cursor {
            Some(cursor) if valid_cursor_token(&cursor) => Some(cursor),
            Some(_) => return Err(NativeClientError::InvalidResponse),
            None => None,
        };
        let mut items = Vec::with_capacity(self.items.len());
        for item in self.items {
            if item.node_id != requested.as_str()
                || !is_canonical_hex(&item.container_id, CONTAINER_ID_HEX_LEN)
            {
                return Err(NativeClientError::InvalidResponse);
            }
            items.push(ContainerItem {
                container_id: item.container_id,
                display_name: item.display_name,
                image_reference: item.image_reference,
                state: item.state,
                status: item.status,
                created_at: parse_timestamp(&item.created_at)?,
                node_id: item.node_id,
            });
        }
        Ok(ContainerPage { items, next_cursor })
    }
}
