use serde::{Deserialize, Serialize};
use tauri_plugin_store::StoreExt;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub refresh_interval: Option<u32>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            refresh_interval: Some(5000),
        }
    }
}

#[tauri::command]
pub async fn load_app_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    let store = app
        .store("settings.json")
        .map_err(|e| format!("store error: {e}"))?;

    let settings: AppSettings = store
        .get("appSettings")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    Ok(settings)
}
