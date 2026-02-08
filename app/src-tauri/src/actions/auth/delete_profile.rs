use tauri_plugin_store::StoreExt;

use super::ConnectionProfile;

#[tauri::command]
pub async fn delete_profile(app: tauri::AppHandle, profile_id: String) -> Result<(), String> {
    let store = app
        .store("profiles.json")
        .map_err(|e| format!("store error: {e}"))?;

    let mut profiles: Vec<ConnectionProfile> = store
        .get("profiles")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    profiles.retain(|p| p.id != profile_id);

    // If deleting the active profile, clear the pointer
    let active_id: Option<String> = store
        .get("activeProfileId")
        .and_then(|v| v.as_str().map(String::from));

    let new_active = if active_id.as_deref() == Some(&profile_id) {
        None
    } else {
        active_id
    };

    store.set(
        "profiles",
        serde_json::to_value(&profiles).map_err(|e| format!("serialize error: {e}"))?,
    );
    store.set("activeProfileId", serde_json::json!(new_active));
    store.save().map_err(|e| format!("store save error: {e}"))?;

    Ok(())
}
