//! Log stream connects through the session's Rust-held access credential.

use kontainer_native_client::{LogConnectError, LogOpenRequest, LogReceiver};
use uuid::Uuid;

use super::{inventory::InventoryError, SessionManager};

impl SessionManager {
    /// Upgrade to one service's log stream with the active access credential, refreshing once
    /// on a 401 like every other authenticated call.
    pub(crate) async fn connect_logs(
        &self,
        profile_id: Uuid,
        request: LogOpenRequest,
    ) -> Result<Box<dyn LogReceiver>, InventoryError> {
        self.authenticated(
            profile_id,
            &request,
            |controller, access, cancellation, request| {
                controller.connect_logs(access, request, cancellation)
            },
        )
        .await
    }
}

/// Whether a follower should try again: an unreachable controller or manager is worth another
/// try; anything else (sign-in needed, the session's limit, a bad request) ends the stream with
/// that code.
pub(crate) fn log_connect_error(error: InventoryError) -> LogConnectError {
    match error {
        InventoryError::ControllerUnavailable
        | InventoryError::AgentUnavailable
        | InventoryError::AgentTimeout
        | InventoryError::QuorumUnavailable => LogConnectError::Transient,
        other => LogConnectError::Terminal(
            serde_json::to_value(other)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "invalid_response".to_owned()),
        ),
    }
}
