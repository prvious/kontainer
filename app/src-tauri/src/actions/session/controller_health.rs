use tauri::State;

use crate::session::manager::SessionManager;
use crate::session::models::ProbeView;

/// Readiness of the signed-in controller for the titlebar dot and the status
/// bar; `None` while signed out.
#[tauri::command]
pub async fn controller_health(state: State<'_, SessionManager>) -> Result<Option<ProbeView>, ()> {
    Ok(state.health().await)
}
