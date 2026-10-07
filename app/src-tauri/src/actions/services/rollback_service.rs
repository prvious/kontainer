use kontainer_native_client::ServiceAction;
use tauri::State;
use uuid::Uuid;

use crate::session::manager::{AdmittedView, InventoryError, SessionManager};

/// Admit one rollback to the previous spec based on `version`.
#[tauri::command]
pub async fn rollback_service(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    service_id: String,
    version: u64,
    idempotency_key: String,
) -> Result<AdmittedView, InventoryError> {
    state
        .service_action(
            profile_id,
            &service_id,
            ServiceAction::Rollback,
            version,
            &idempotency_key,
        )
        .await
}
