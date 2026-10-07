use tauri::State;
use uuid::Uuid;

use crate::session::manager::{
    AdmittedView, ContainerActionOptionsInput, ContainerTargetInput, InventoryError, SessionManager,
};

/// Admit one lifecycle action; poll the returned operation.
#[tauri::command]
pub async fn container_action(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    node_id: String,
    container_id: String,
    action: String,
    options: Option<ContainerActionOptionsInput>,
    idempotency_key: String,
) -> Result<AdmittedView, InventoryError> {
    let target = ContainerTargetInput {
        node_id,
        container_id,
    };
    state
        .container_action(
            profile_id,
            &target,
            &action,
            options.unwrap_or_default(),
            &idempotency_key,
        )
        .await
}
