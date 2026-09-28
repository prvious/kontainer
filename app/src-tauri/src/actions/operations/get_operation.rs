use tauri::State;
use uuid::Uuid;

use crate::session::manager::{InventoryError, OperationView, SessionManager};

/// Poll one operation the signed-in principal admitted.
#[tauri::command]
pub async fn get_operation(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    operation_id: String,
) -> Result<OperationView, InventoryError> {
    state.operation(profile_id, &operation_id).await
}
