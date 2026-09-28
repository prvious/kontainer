use tauri::State;
use uuid::Uuid;

use crate::session::manager::{InventoryError, OperationView, SessionManager};

/// Cancel an operation that has not started; `conflict` once it has.
#[tauri::command]
pub async fn cancel_operation(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    operation_id: String,
) -> Result<OperationView, InventoryError> {
    state.cancel_operation(profile_id, &operation_id).await
}
