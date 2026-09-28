use tauri::State;
use uuid::Uuid;

use crate::session::manager::{ContainerListView, InventoryError, SessionManager};

/// Containers on every answering node (or `nodeId` alone), with one outcome per member.
#[tauri::command]
pub async fn list_containers(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    node_id: Option<String>,
) -> Result<ContainerListView, InventoryError> {
    state.containers(profile_id, node_id.as_deref()).await
}
