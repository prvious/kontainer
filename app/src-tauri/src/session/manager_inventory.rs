//! Authenticated, read-only inventory through the session's Rust-held access credential.

use kontainer_native_client::{
    AccessCredential, ContainerPage, ContainerPageRequest, NativeClientError, NodeId,
};
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{ActiveSession, SessionManager};
use crate::session::{controller::Controller, errors::SessionError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InventoryError {
    Unauthenticated,
    ReloginRequired,
    VaultUnavailable,
    VaultWriteFailed,
    LocalStateFailed,
    CleanupUncertain,
    InvalidNodeId,
    InvalidPagination,
    NodeNotFound,
    Forbidden,
    ControllerUnavailable,
    AgentUnavailable,
    AgentTimeout,
    InvalidResponse,
}

#[cfg(test)]
impl InventoryError {
    pub(crate) const ALL: &'static [Self] = &[
        Self::Unauthenticated,
        Self::ReloginRequired,
        Self::VaultUnavailable,
        Self::VaultWriteFailed,
        Self::LocalStateFailed,
        Self::CleanupUncertain,
        Self::InvalidNodeId,
        Self::InvalidPagination,
        Self::NodeNotFound,
        Self::Forbidden,
        Self::ControllerUnavailable,
        Self::AgentUnavailable,
        Self::AgentTimeout,
        Self::InvalidResponse,
    ];
}

impl From<SessionError> for InventoryError {
    fn from(error: SessionError) -> Self {
        match error {
            SessionError::ReloginRequired => Self::ReloginRequired,
            SessionError::VaultUnavailable => Self::VaultUnavailable,
            SessionError::VaultWriteFailed => Self::VaultWriteFailed,
            SessionError::LocalStateFailed => Self::LocalStateFailed,
            SessionError::CleanupUncertain | SessionError::LocalCleanupFailed => {
                Self::CleanupUncertain
            }
            SessionError::ControllerUnavailable => Self::ControllerUnavailable,
            SessionError::InvalidResponse => Self::InvalidResponse,
            _ => Self::Unauthenticated,
        }
    }
}

impl From<NativeClientError> for InventoryError {
    fn from(error: NativeClientError) -> Self {
        match error {
            NativeClientError::InvalidNodeId => Self::InvalidNodeId,
            NativeClientError::InvalidPagination => Self::InvalidPagination,
            NativeClientError::Unauthorized => Self::ReloginRequired,
            NativeClientError::Forbidden => Self::Forbidden,
            NativeClientError::NotFound => Self::NodeNotFound,
            NativeClientError::AgentUnavailable => Self::AgentUnavailable,
            NativeClientError::AgentTimeout => Self::AgentTimeout,
            NativeClientError::Unavailable
            | NativeClientError::DeadlineExceeded
            | NativeClientError::ServiceUnavailable
            | NativeClientError::GatewayTimeout
            | NativeClientError::Cancelled => Self::ControllerUnavailable,
            _ => Self::InvalidResponse,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ContainerPageView {
    items: Vec<ContainerItemView>,
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContainerItemView {
    container_id: String,
    display_name: String,
    image_reference: String,
    state: kontainer_native_client::ContainerState,
    status: String,
    created_at: String,
    node_id: String,
}

impl From<ContainerPage> for ContainerPageView {
    fn from(page: ContainerPage) -> Self {
        Self {
            items: page
                .items
                .into_iter()
                .map(|item| ContainerItemView {
                    container_id: item.container_id,
                    display_name: item.display_name,
                    image_reference: item.image_reference,
                    state: item.state,
                    status: item.status,
                    created_at: item.created_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                    node_id: item.node_id,
                })
                .collect(),
            next_cursor: page.next_cursor,
        }
    }
}

enum ReadKind {
    Discover,
    Containers {
        node: NodeId,
        page: ContainerPageRequest,
    },
}

enum ReadResult {
    Node(Option<NodeId>),
    Page(ContainerPage),
}

/// An in-flight read temporarily owns access without cloning it. Cancellation
/// restores an accepted credential; after a 401 it is discarded before rotation.
struct ReadCustody<'a> {
    manager: &'a SessionManager,
    active: Option<ActiveSession>,
}

impl Drop for ReadCustody<'_> {
    fn drop(&mut self) {
        if let Some(active) = self.active.take() {
            let mut slots = self.manager.slots();
            if slots.active.is_none() {
                slots.active = Some(active);
            }
        }
    }
}

async fn read_once(
    controller: &dyn Controller,
    access: &AccessCredential,
    kind: &ReadKind,
    cancellation: &CancellationToken,
) -> Result<ReadResult, NativeClientError> {
    match kind {
        ReadKind::Discover => controller
            .discover_node(access, cancellation)
            .await
            .map(ReadResult::Node),
        ReadKind::Containers { node, page } => controller
            .list_containers(access, node, page, cancellation)
            .await
            .map(ReadResult::Page),
    }
}

impl SessionManager {
    pub(crate) async fn configured_node(
        &self,
        profile_id: Uuid,
    ) -> Result<Option<String>, InventoryError> {
        match self.read_inventory(profile_id, ReadKind::Discover).await? {
            ReadResult::Node(node) => Ok(node.map(|node| node.to_string())),
            ReadResult::Page(_) => unreachable!("discovery read returned a page"),
        }
    }

    pub(crate) async fn container_inventory(
        &self,
        profile_id: Uuid,
        node_id: &str,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<ContainerPageView, InventoryError> {
        let node = NodeId::parse(node_id)?;
        let page = ContainerPageRequest { limit, cursor };
        match self
            .read_inventory(profile_id, ReadKind::Containers { node, page })
            .await?
        {
            ReadResult::Page(page) => Ok(page.into()),
            ReadResult::Node(_) => unreachable!("container read returned a node"),
        }
    }

    async fn read_inventory(
        &self,
        profile_id: Uuid,
        kind: ReadKind,
    ) -> Result<ReadResult, InventoryError> {
        let _ops = self.ops.lock().await;
        if let Some(error) = self.custody_error() {
            return Err(error.into());
        }
        let mut custody = ReadCustody {
            manager: self,
            active: self.slots().active.take(),
        };
        let Some(active) = custody.active.as_ref() else {
            return Err(InventoryError::Unauthenticated);
        };
        if active.view.profile_id != profile_id {
            return Err(InventoryError::Unauthenticated);
        }
        let controller = self
            .controllers
            .for_origin(&active.view.origin)
            .map_err(InventoryError::from)?;
        let cancellation = CancellationToken::new();
        let first = read_once(controller.as_ref(), &active.access, &kind, &cancellation).await;
        if !matches!(&first, Err(NativeClientError::Unauthorized)) {
            return first.map_err(InventoryError::from);
        }

        // A rejected access credential is never published again. The refresh
        // path marks the anchor pending before sending its one-use credential.
        custody.active = None;
        let anchor = self
            .load_anchor()
            .await?
            .ok_or(InventoryError::LocalStateFailed)?;
        if anchor.profile_id != profile_id {
            return Err(InventoryError::Unauthenticated);
        }
        let refreshed = self.restore_ready(anchor).await?;
        custody.active = self.slots().active.take();
        let active = custody
            .active
            .as_ref()
            .ok_or(InventoryError::LocalStateFailed)?;
        let second = read_once(controller.as_ref(), &active.access, &kind, &cancellation).await;
        if !matches!(&second, Err(NativeClientError::Unauthorized)) {
            return second.map_err(InventoryError::from);
        }

        custody.active = None;
        let anchor = self
            .load_anchor()
            .await?
            .ok_or(InventoryError::LocalStateFailed)?;
        Err(self
            .invalidate_to_relogin(&anchor, refreshed.label)
            .await
            .into())
    }
}
