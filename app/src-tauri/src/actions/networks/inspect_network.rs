use tauri::State;
use uuid::Uuid;

use crate::session::manager::{AttachedContainerView, InventoryError, SessionManager};

/// The containers attached to one network: on `nodeId` for a local one, cluster-wide otherwise.
#[tauri::command]
pub async fn inspect_network(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    network_id: String,
    node_id: Option<String>,
) -> Result<Vec<AttachedContainerView>, InventoryError> {
    state
        .network_containers(profile_id, &network_id, node_id.as_deref())
        .await
}
