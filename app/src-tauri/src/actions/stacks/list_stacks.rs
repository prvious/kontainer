use tauri::State;
use uuid::Uuid;

use crate::session::manager::{InventoryError, SessionManager, StackListView};

/// Every stack with its task counts and container figures.
#[tauri::command]
pub async fn list_stacks(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
) -> Result<StackListView, InventoryError> {
    state.stacks(profile_id).await
}
