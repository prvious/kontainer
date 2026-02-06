use tauri_plugin_store::StoreExt;

#[tauri::command]
pub fn delete_session(app: tauri::AppHandle) -> Result<(), String> {
    let store = app
        .store("session.json")
        .map_err(|e| format!("store error: {e}"))?;

    let user_id = store
        .get("userId")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();

    if !user_id.is_empty() {
        if let Ok(entry) = keyring::Entry::new("dev.devoper.app", &user_id) {
            let _ = entry.delete_credential();
        }
    }

    store.clear();
    store.save().map_err(|e| format!("store save error: {e}"))?;

    Ok(())
}
