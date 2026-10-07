use tauri::State;
use uuid::Uuid;

use crate::session::errors::SessionError;
use crate::session::manager::SessionManager;

/// Remove one saved connection from the login screen.
#[tauri::command]
pub fn session_forget_connection(
    state: State<'_, SessionManager>,
    id: Uuid,
) -> Result<(), SessionError> {
    state.forget_connection(id)
}
