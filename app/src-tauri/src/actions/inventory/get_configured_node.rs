use tauri::State;
use uuid::Uuid;

use crate::session::manager::{InventoryError, SessionManager};

/// Resolve the configured node through the authenticated controller session.
#[tauri::command]
pub async fn get_configured_node(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
) -> Result<Option<String>, InventoryError> {
    state.configured_node(profile_id).await
}
