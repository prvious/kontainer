use tauri::State;
use uuid::Uuid;

use crate::session::manager::{AdmittedView, InventoryError, SessionManager};

/// Admit one anonymous pull on `nodeIds`; the operation's progress carries per-node state.
#[tauri::command]
pub async fn pull_image(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    reference: String,
    node_ids: Vec<String>,
    idempotency_key: String,
) -> Result<AdmittedView, InventoryError> {
    state
        .pull_image(profile_id, &reference, &node_ids, &idempotency_key)
        .await
}
