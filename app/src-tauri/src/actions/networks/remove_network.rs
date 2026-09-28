use tauri::State;
use uuid::Uuid;

use crate::session::manager::{AdmittedView, InventoryError, SessionManager};

/// Admit one network removal; attached containers fail the operation with their names.
#[tauri::command]
pub async fn remove_network(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    network_id: String,
    node_id: Option<String>,
    idempotency_key: String,
) -> Result<AdmittedView, InventoryError> {
    state
        .remove_network(
            profile_id,
            &network_id,
            node_id.as_deref(),
            &idempotency_key,
        )
        .await
}
