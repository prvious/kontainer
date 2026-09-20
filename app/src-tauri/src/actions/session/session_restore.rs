use tauri::State;

use crate::session::errors::SessionError;
use crate::session::manager::SessionManager;
use crate::session::models::SessionView;

/// Restore the stored session, rotating the refresh credential once.
///
/// Returns `None` when no profile session exists. Failure codes tell the UI
/// exactly which state to render; no credential or raw error text is exposed.
#[tauri::command]
pub async fn session_restore(
    state: State<'_, SessionManager>,
) -> Result<Option<SessionView>, SessionError> {
    state.restore().await
}
