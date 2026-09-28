use tauri::State;
use uuid::Uuid;

use crate::session::manager::{AdmittedView, CreateVolumeInput, InventoryError, SessionManager};

/// Admit one `local` volume on one node; poll the returned operation.
#[tauri::command]
pub async fn create_volume(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    volume: CreateVolumeInput,
    idempotency_key: String,
) -> Result<AdmittedView, InventoryError> {
    state
        .create_volume(profile_id, &volume, &idempotency_key)
        .await
}
