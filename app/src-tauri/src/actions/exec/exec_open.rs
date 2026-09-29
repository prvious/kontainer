use kontainer_native_client::{ExecOpenRequest, NodeId};
use tauri::{ipc::Channel, State};
use uuid::Uuid;

use crate::session::exec_sessions::{ExecSessions, ExecStreamEvent};
use crate::session::manager::{InventoryError, SessionManager};

/// Open a shell in one container; its events arrive on `channel` until it closes.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn exec_open(
    manager: State<'_, SessionManager>,
    sessions: State<'_, ExecSessions>,
    profile_id: Uuid,
    node_id: String,
    container_id: String,
    shell: Option<String>,
    cols: u32,
    rows: u32,
    channel: Channel<ExecStreamEvent>,
) -> Result<Uuid, InventoryError> {
    let request = ExecOpenRequest {
        node_id: NodeId::parse(&node_id).map_err(|_| InventoryError::InvalidNodeId)?,
        container_id,
        shell: shell.filter(|shell| !shell.trim().is_empty()),
        cols,
        rows,
    };
    request
        .validate()
        .map_err(|_| InventoryError::InvalidRequest)?;
    let connection = manager.connect_exec(profile_id, request).await?;
    Ok(sessions.open(connection, channel))
}
