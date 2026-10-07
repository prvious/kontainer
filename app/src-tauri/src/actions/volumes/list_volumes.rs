use tauri::State;
use uuid::Uuid;

use crate::session::manager::{InventoryError, SessionManager, VolumeListView};

/// Volumes on every answering node (or `nodeId` alone, optionally one `name`), one row per node.
#[tauri::command]
pub async fn list_volumes(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    node_id: Option<String>,
    name: Option<String>,
) -> Result<VolumeListView, InventoryError> {
    state
        .volumes(profile_id, node_id.as_deref(), name.as_deref())
        .await
}
