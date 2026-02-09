use tauri_plugin_store::StoreExt;

use super::load_app_settings::AppSettings;

#[tauri::command]
pub async fn save_app_settings(
    app: tauri::AppHandle,
    settings: AppSettings,
) -> Result<(), String> {
    let store = app
        .store("settings.json")
        .map_err(|e| format!("store error: {e}"))?;

    store.set(
        "appSettings",
        serde_json::to_value(&settings).map_err(|e| format!("serialize error: {e}"))?,
    );
    store.save().map_err(|e| format!("store save error: {e}"))?;

    Ok(())
}
