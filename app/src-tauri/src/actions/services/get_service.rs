use tauri::State;
use uuid::Uuid;

use crate::session::manager::{InventoryError, ServiceDetailView, SessionManager};

/// One service with its newest tasks and the newest task error.
#[tauri::command]
pub async fn get_service(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    service_id: String,
) -> Result<ServiceDetailView, InventoryError> {
    state.service(profile_id, &service_id).await
}
