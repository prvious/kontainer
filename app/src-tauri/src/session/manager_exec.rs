//! Exec connects through the session's Rust-held access credential.

use kontainer_native_client::{ExecConnection, ExecOpenRequest};
use uuid::Uuid;

use super::{inventory::InventoryError, SessionManager};

impl SessionManager {
    /// Upgrade to a shell with the active access credential, refreshing once on a 401 like every
    /// other authenticated call.
    pub(crate) async fn connect_exec(
        &self,
        profile_id: Uuid,
        request: ExecOpenRequest,
    ) -> Result<ExecConnection, InventoryError> {
        self.authenticated(
            profile_id,
            &request,
            |controller, access, cancellation, request| {
                controller.connect_exec(access, request, cancellation)
            },
        )
        .await
    }
}
