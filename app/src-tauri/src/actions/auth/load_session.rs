use tauri_plugin_store::StoreExt;

use super::StoredSession;

#[tauri::command]
pub fn load_session(app: tauri::AppHandle) -> Result<Option<StoredSession>, String> {
    let store = app
        .store("session.json")
        .map_err(|e| format!("store error: {e}"))?;

    let user_id = match store.get("userId") {
        Some(val) => val.as_str().unwrap_or_default().to_string(),
        None => return Ok(None),
    };

    if user_id.is_empty() {
        return Ok(None);
    }

    let entry = keyring::Entry::new("dev.devoper.app", &user_id)
        .map_err(|e| format!("keyring entry error: {e}"))?;

    let token = match entry.get_password() {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };

    let user_name = store
        .get("userName")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let user_email = store
        .get("userEmail")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let user_avatar_url = store
        .get("userAvatarUrl")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let user_provider = store
        .get("userProvider")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let expires_at = store.get("expiresAt").and_then(|v| v.as_u64()).unwrap_or(0);
    let agent_url = store
        .get("agentUrl")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();

    Ok(Some(StoredSession {
        user_id,
        user_name,
        user_email,
        user_avatar_url,
        user_provider,
        token,
        expires_at,
        agent_url,
    }))
}
