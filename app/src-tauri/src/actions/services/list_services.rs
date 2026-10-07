use tauri::State;
use uuid::Uuid;

use crate::session::manager::{InventoryError, ServiceListView, SessionManager};

/// Every Swarm service (or one stack's) with running replicas.
#[tauri::command]
pub async fn list_services(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    stack: Option<String>,
) -> Result<ServiceListView, InventoryError> {
    state.services(profile_id, stack.as_deref()).await
}
