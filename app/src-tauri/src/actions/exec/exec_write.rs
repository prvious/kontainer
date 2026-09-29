use kontainer_native_client::ExecInputFrame;
use tauri::State;
use uuid::Uuid;

use crate::session::exec_sessions::ExecSessions;

/// Keystrokes and pastes for one session; a session that ended ignores them.
#[tauri::command]
pub async fn exec_write(
    sessions: State<'_, ExecSessions>,
    session_id: Uuid,
    data: String,
) -> Result<(), ()> {
    sessions
        .send(session_id, ExecInputFrame::Input(data.into_bytes()))
        .await;
    Ok(())
}
