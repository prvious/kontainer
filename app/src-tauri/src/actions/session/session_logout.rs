use tauri::State;

use crate::session::errors::SessionError;
use crate::session::manager::SessionManager;

/// Revoke the server session before clearing local custody.
///
/// `Ok` means the desktop is logged out and revocation was confirmed (or the
/// family was provably gone). `local_cleanup_failed` means it is logged out
/// but a local credential could not be deleted; retrying the logout is local
/// only. `revoke_failed` means custody was preserved because the server
/// session may still be alive. `cleanup_uncertain` means a non-restorable
/// anchor exists that no reachable credential can revoke; only a fresh login
/// resolves it. Vault failures after revocation propagate unchanged.
#[tauri::command]
pub async fn session_logout(state: State<'_, SessionManager>) -> Result<(), SessionError> {
    state.logout().await
}
