use tauri::State;

use crate::session::errors::SessionError;
use crate::session::manager::SessionManager;

/// Cancel the active login attempt and wait for its request work to stop.
#[tauri::command]
pub async fn session_cancel_login(state: State<'_, SessionManager>) -> Result<(), SessionError> {
    state.cancel_login().await
}
