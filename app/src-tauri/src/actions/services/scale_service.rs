use kontainer_native_client::ServiceAction;
use tauri::State;
use uuid::Uuid;

use crate::session::manager::{AdmittedView, InventoryError, SessionManager};

/// Admit one scale based on `version`; the operation tracks convergence.
#[tauri::command]
pub async fn scale_service(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    service_id: String,
    replicas: u64,
    version: u64,
    idempotency_key: String,
) -> Result<AdmittedView, InventoryError> {
    state
        .service_action(
            profile_id,
            &service_id,
            ServiceAction::Scale { replicas },
            version,
            &idempotency_key,
        )
        .await
}
