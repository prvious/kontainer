use tauri_plugin_store::StoreExt;

use super::ConnectionProfile;

#[tauri::command]
pub async fn delete_session(app: tauri::AppHandle) -> Result<(), String> {
    let store = app
        .store("profiles.json")
        .map_err(|e| format!("store error: {e}"))?;

    let mut profiles: Vec<ConnectionProfile> = store
        .get("profiles")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let active_id: Option<String> = store
        .get("activeProfileId")
        .and_then(|v| v.as_str().map(String::from));

    // Remove the active profile
    if let Some(ref id) = active_id {
        profiles.retain(|p| &p.id != id);
    }

    store.set(
        "profiles",
        serde_json::to_value(&profiles).map_err(|e| format!("serialize error: {e}"))?,
    );
    store.set("activeProfileId", serde_json::json!(null));
    store.save().map_err(|e| format!("store save error: {e}"))?;

    Ok(())
}
