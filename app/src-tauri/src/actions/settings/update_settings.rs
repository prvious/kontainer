use tauri::State;
use uuid::Uuid;

use crate::session::manager::{ControllerSettingsView, InventoryError, SessionManager};

/// Change the metrics retention (1–365 days).
#[tauri::command]
pub async fn update_settings(
    state: State<'_, SessionManager>,
    profile_id: Uuid,
    metrics_retention_days: u32,
) -> Result<ControllerSettingsView, InventoryError> {
    state
        .update_controller_settings(profile_id, metrics_retention_days)
        .await
}
