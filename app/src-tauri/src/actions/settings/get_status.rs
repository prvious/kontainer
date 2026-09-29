use tauri::State;
use uuid::Uuid;

use crate::session::manager::{ControllerStatusView, InventoryError, SessionManager};

/// The controller's version, log filter, and start time.
#[tauri::command]
pub async fn get_status(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
) -> Result<ControllerStatusView, InventoryError> {
    state.controller_status(profile_id).await
}
