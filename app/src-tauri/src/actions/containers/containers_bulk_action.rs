use tauri::State;
use uuid::Uuid;

use crate::session::manager::{AdmittedView, ContainerTargetInput, InventoryError, SessionManager};

/// Admit one action across up to 200 containers as a single operation.
#[tauri::command]
pub async fn containers_bulk_action(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    action: String,
    targets: Vec<ContainerTargetInput>,
    force: Option<bool>,
    idempotency_key: String,
) -> Result<AdmittedView, InventoryError> {
    state
        .containers_bulk_action(
            profile_id,
            &action,
            &targets,
            force.unwrap_or(false),
            &idempotency_key,
        )
        .await
}
