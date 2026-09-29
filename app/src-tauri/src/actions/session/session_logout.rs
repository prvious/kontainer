use tauri::State;

use crate::session::errors::SessionError;
use crate::session::log_streams::LogStreams;
use crate::session::manager::SessionManager;
use crate::session::models::ConnectionSummary;

/// Revoke the server session before clearing local custody.
///
/// `summary` is the cluster size React last saw; it is cached on the saved
/// connection for the switcher subtitle before signing out.
///
/// `Ok` means the desktop is logged out and revocation was confirmed (or the
/// family was provably gone). `local_cleanup_failed` means it is logged out
/// but a local credential could not be deleted; retrying the logout is local
/// only. `revoke_failed` means custody was preserved because the server
/// session may still be alive. `cleanup_uncertain` means a non-restorable
/// anchor exists that no reachable credential can revoke; only a fresh login
/// resolves it. Vault failures after revocation propagate unchanged.
#[tauri::command]
pub async fn session_logout(
    state: State<'_, SessionManager>,
    streams: State<'_, LogStreams>,
    summary: Option<ConnectionSummary>,
) -> Result<(), SessionError> {
    // No stream outlives the session it was opened with.
    streams.cancel_all();
    if let (Some(summary), Some(origin)) = (summary, state.active_origin()) {
        // Best effort: a summary that cannot be cached never blocks sign-out.
        let _ = state.record_summary(&origin, summary);
    }
    state.logout().await
}
