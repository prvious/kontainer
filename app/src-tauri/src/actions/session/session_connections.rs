use tauri::State;

use crate::session::manager::SessionManager;
use crate::session::models::SavedConnection;

/// Saved connections, most recent first (at most ten). Non-secret metadata.
#[tauri::command]
pub fn session_connections(state: State<'_, SessionManager>) -> Vec<SavedConnection> {
    state.connections()
}
