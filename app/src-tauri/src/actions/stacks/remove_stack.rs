use tauri::State;
use uuid::Uuid;

use crate::session::manager::{AdmittedView, InventoryError, SessionManager};

/// Admit one stack removal: services, then its networks; volumes are kept.
#[tauri::command]
pub async fn remove_stack(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    name: String,
    idempotency_key: String,
) -> Result<AdmittedView, InventoryError> {
    state
        .remove_stack(profile_id, &name, &idempotency_key)
        .await
}
