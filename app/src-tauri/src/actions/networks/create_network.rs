use tauri::State;
use uuid::Uuid;

use crate::session::manager::{AdmittedView, CreateNetworkInput, InventoryError, SessionManager};

/// Admit one overlay (on the manager) or bridge (on `nodeId`) network; poll the operation.
#[tauri::command]
pub async fn create_network(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    network: CreateNetworkInput,
    idempotency_key: String,
) -> Result<AdmittedView, InventoryError> {
    state
        .create_network(profile_id, &network, &idempotency_key)
        .await
}
