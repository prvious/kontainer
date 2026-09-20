use tauri::State;

use crate::session::errors::SessionError;
use crate::session::manager::SessionManager;
use crate::session::models::SessionView;

/// Retry the local commit of a successor that was issued but not persisted.
#[tauri::command]
pub async fn session_retry_commit(
    state: State<'_, SessionManager>,
) -> Result<SessionView, SessionError> {
    state.retry_commit().await
}
