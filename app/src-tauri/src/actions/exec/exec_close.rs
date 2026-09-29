use tauri::State;
use uuid::Uuid;

use crate::session::exec_sessions::ExecSessions;

/// Ask the shell to exit; its exit and the close arrive as events.
#[tauri::command]
pub async fn exec_close(sessions: State<'_, ExecSessions>, session_id: Uuid) -> Result<(), ()> {
    sessions.close(session_id).await;
    Ok(())
}
