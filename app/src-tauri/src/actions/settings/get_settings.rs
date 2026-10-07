use tauri::State;
use uuid::Uuid;

use crate::session::manager::{ControllerSettingsView, InventoryError, SessionManager};

/// Controller-wide settings (metrics retention).
#[tauri::command]
pub async fn get_settings(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
) -> Result<ControllerSettingsView, InventoryError> {
    state.controller_settings(profile_id).await
}
