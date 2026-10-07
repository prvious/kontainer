use kontainer_native_client::ExecInputFrame;
use tauri::State;
use uuid::Uuid;

use crate::session::exec_sessions::ExecSessions;

/// The terminal's new size in columns and rows.
#[tauri::command]
pub async fn exec_resize(
    sessions: State<'_, ExecSessions>,
    session_id: Uuid,
    cols: u32,
    rows: u32,
) -> Result<(), ()> {
    sessions
        .send(session_id, ExecInputFrame::Resize { cols, rows })
        .await;
    Ok(())
}
