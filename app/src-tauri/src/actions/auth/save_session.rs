use tauri_plugin_store::StoreExt;

use super::SaveSessionInput;

#[tauri::command]
pub async fn save_session(app: tauri::AppHandle, input: SaveSessionInput) -> Result<(), String> {
    let store = app
        .store("session.json")
        .map_err(|e| format!("store error: {e}"))?;

    store.set("userId", serde_json::json!(input.user_id));
    store.set("userName", serde_json::json!(input.user_name));
    store.set("userEmail", serde_json::json!(input.user_email));
    store.set("userAvatarUrl", serde_json::json!(input.user_avatar_url));
    store.set("userProvider", serde_json::json!(input.user_provider));
    store.set("token", serde_json::json!(input.token));
    store.set("expiresAt", serde_json::json!(input.expires_at));
    store.set("agentUrl", serde_json::json!(input.agent_url));

    store.save().map_err(|e| format!("store save error: {e}"))?;

    Ok(())
}
