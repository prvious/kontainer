use tauri::State;
use uuid::Uuid;

use crate::session::manager::{InventoryError, NodeListView, SessionManager};

/// Every member node with its status and latest sample.
#[tauri::command]
pub async fn list_nodes(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
) -> Result<NodeListView, InventoryError> {
    state.nodes(profile_id).await
}
