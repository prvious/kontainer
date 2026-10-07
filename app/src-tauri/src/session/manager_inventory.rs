//! Authenticated controller reads through the session's Rust-held access credential.
//!
//! Every call runs through [`SessionManager::authenticated`]: one attempt, and on a 401 one
//! refresh-then-retry of the same call before the session is invalidated.

use kontainer_native_client::{AccessCredential, NativeClientError};
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{ActiveSession, SessionManager};
use crate::session::{
    controller::{Controller, ControllerFuture},
    errors::SessionError,
};

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
    InvalidRequest,
    NodeNotFound,
    NotFound,
    Conflict,
    TooManyContainers,
    Forbidden,
    ControllerUnavailable,
    AgentUnavailable,
    AgentTimeout,
    QuorumUnavailable,
    NotAManager,
    VersionConflict,
    LimitReached,
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
        Self::InvalidRequest,
        Self::NodeNotFound,
        Self::NotFound,
        Self::Conflict,
        Self::TooManyContainers,
        Self::Forbidden,
        Self::ControllerUnavailable,
        Self::AgentUnavailable,
        Self::AgentTimeout,
        Self::QuorumUnavailable,
        Self::NotAManager,
        Self::VersionConflict,
        Self::LimitReached,
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
            NativeClientError::InvalidRequest => Self::InvalidRequest,
            NativeClientError::Unauthorized => Self::ReloginRequired,
            NativeClientError::Forbidden => Self::Forbidden,
            NativeClientError::NotFound => Self::NodeNotFound,
            NativeClientError::Conflict => Self::Conflict,
            NativeClientError::TooManyContainers => Self::TooManyContainers,
            NativeClientError::AgentUnavailable => Self::AgentUnavailable,
            NativeClientError::AgentTimeout => Self::AgentTimeout,
            NativeClientError::QuorumUnavailable { .. } => Self::QuorumUnavailable,
            NativeClientError::NotAManager => Self::NotAManager,
            NativeClientError::VersionConflict => Self::VersionConflict,
            NativeClientError::LimitReached => Self::LimitReached,
            NativeClientError::Unavailable
            | NativeClientError::DeadlineExceeded
            | NativeClientError::ServiceUnavailable
            | NativeClientError::GatewayTimeout
            | NativeClientError::Cancelled => Self::ControllerUnavailable,
            _ => Self::InvalidResponse,
        }
    }
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

impl SessionManager {
    /// Run `call` with the active profile's access credential, rotating once on a 401. The call
    /// borrows its inputs from `arguments` so the same call can repeat with the successor.
    pub(crate) async fn authenticated<A, T, F>(
        &self,
        profile_id: Uuid,
        arguments: &A,
        call: F,
    ) -> Result<T, InventoryError>
    where
        A: ?Sized + Sync,
        F: for<'a> Fn(
            &'a dyn Controller,
            &'a AccessCredential,
            &'a CancellationToken,
            &'a A,
        ) -> ControllerFuture<'a, Result<T, NativeClientError>>,
    {
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
        let first = call(
            controller.as_ref(),
            &active.access,
            &cancellation,
            arguments,
        )
        .await;
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
        self.restore_ready(anchor).await?;
        custody.active = self.slots().active.take();
        let active = custody
            .active
            .as_ref()
            .ok_or(InventoryError::LocalStateFailed)?;
        let second = call(
            controller.as_ref(),
            &active.access,
            &cancellation,
            arguments,
        )
        .await;
        if !matches!(&second, Err(NativeClientError::Unauthorized)) {
            return second.map_err(InventoryError::from);
        }

        custody.active = None;
        let anchor = self
            .load_anchor()
            .await?
            .ok_or(InventoryError::LocalStateFailed)?;
        Err(self.invalidate_to_relogin(&anchor).await.into())
    }
}
