use tauri::State;
use uuid::Uuid;

use crate::session::manager::{AdmittedView, InventoryError, SessionManager};

/// Admit one volume removal; a mounted volume fails the operation with the containers' names.
#[tauri::command]
pub async fn remove_volume(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    node_id: String,
    name: String,
    idempotency_key: String,
) -> Result<AdmittedView, InventoryError> {
    state
        .remove_volume(profile_id, &node_id, &name, &idempotency_key)
        .await
}
