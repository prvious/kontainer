use tauri_plugin_store::StoreExt;

use super::ConnectionProfile;

#[tauri::command]
pub async fn set_active_profile(app: tauri::AppHandle, profile_id: String) -> Result<(), String> {
    let store = app
        .store("profiles.json")
        .map_err(|e| format!("store error: {e}"))?;

    let profiles: Vec<ConnectionProfile> = store
        .get("profiles")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    if !profiles.iter().any(|p| p.id == profile_id) {
        return Err(format!("profile {profile_id} not found"));
    }

    store.set("activeProfileId", serde_json::json!(profile_id));
    store.save().map_err(|e| format!("store save error: {e}"))?;

    Ok(())
}
