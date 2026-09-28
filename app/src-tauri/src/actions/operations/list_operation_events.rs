use tauri::State;
use uuid::Uuid;

use crate::session::manager::{InventoryError, OperationEventView, SessionManager};

/// Up to 100 events after `after`, oldest first.
#[tauri::command]
pub async fn list_operation_events(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    operation_id: String,
    after: u32,
) -> Result<Vec<OperationEventView>, InventoryError> {
    state
        .operation_events(profile_id, &operation_id, after)
        .await
}
