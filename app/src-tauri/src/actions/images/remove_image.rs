use tauri::State;
use uuid::Uuid;

use crate::session::manager::{AdmittedView, ImageNodesInput, InventoryError, SessionManager};

/// Admit one image removal on `nodeIds` (or `"all"` that hold it); poll the returned operation.
#[tauri::command]
pub async fn remove_image(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    image_id: String,
    node_ids: ImageNodesInput,
    force: Option<bool>,
    idempotency_key: String,
) -> Result<AdmittedView, InventoryError> {
    state
        .remove_image(
            profile_id,
            &image_id,
            &node_ids,
            force.unwrap_or(false),
            &idempotency_key,
        )
        .await
}
