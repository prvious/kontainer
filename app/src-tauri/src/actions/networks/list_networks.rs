use tauri::State;
use uuid::Uuid;

use crate::session::manager::{InventoryError, NetworkListView, SessionManager};

/// Swarm-scope networks once, local ones per node, and whether the manager answered.
#[tauri::command]
pub async fn list_networks(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
) -> Result<NetworkListView, InventoryError> {
    state.networks(profile_id).await
}
