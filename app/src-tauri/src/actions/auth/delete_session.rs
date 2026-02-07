use tauri_plugin_store::StoreExt;

#[tauri::command]
pub async fn delete_session(app: tauri::AppHandle) -> Result<(), String> {
    let store = app
        .store("session.json")
        .map_err(|e| format!("store error: {e}"))?;

    store.clear();
    store.save().map_err(|e| format!("store save error: {e}"))?;

    Ok(())
}
