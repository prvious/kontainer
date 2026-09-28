use tauri::State;
use uuid::Uuid;

use crate::session::manager::{ContainerPageView, InventoryError, SessionManager};

/// Read one bounded page with Rust-owned session credentials.
#[tauri::command]
pub async fn list_container_inventory(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    node_id: String,
    cursor: Option<String>,
    limit: u32,
) -> Result<ContainerPageView, InventoryError> {
    state
        .container_inventory(profile_id, &node_id, cursor, limit)
        .await
}
