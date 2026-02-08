use tauri_plugin_store::StoreExt;

use super::{ConnectionProfile, StoredSession};

#[tauri::command]
pub async fn load_session(app: tauri::AppHandle) -> Result<Option<StoredSession>, String> {
    let store = app
        .store("profiles.json")
        .map_err(|e| format!("store error: {e}"))?;

    let profiles: Vec<ConnectionProfile> = store
        .get("profiles")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let active_id: Option<String> = store
        .get("activeProfileId")
        .and_then(|v| v.as_str().map(String::from));

    if let Some(active_id) = active_id {
        if let Some(profile) = profiles.iter().find(|p| p.id == active_id) {
            return Ok(Some(StoredSession {
                user_id: profile.user_id.clone(),
                user_name: profile.user_name.clone(),
                user_email: profile.user_email.clone(),
                user_avatar_url: profile.user_avatar_url.clone(),
                user_provider: profile.user_provider.clone(),
                token: profile.token.clone(),
                expires_at: profile.expires_at,
                agent_url: profile.agent_url.clone(),
            }));
        }
    }

    // Fallback: try legacy session.json for migration
    if let Ok(legacy_store) = app.store("session.json") {
        let user_id = match legacy_store.get("userId") {
            Some(val) => val.as_str().unwrap_or_default().to_string(),
            None => return Ok(None),
        };

        if user_id.is_empty() {
            return Ok(None);
        }

        let token = match legacy_store.get("token") {
            Some(val) => val.as_str().unwrap_or_default().to_string(),
            None => return Ok(None),
        };

        if token.is_empty() {
            return Ok(None);
        }

        let user_name = legacy_store
            .get("userName")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let user_email = legacy_store
            .get("userEmail")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let user_avatar_url = legacy_store
            .get("userAvatarUrl")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let user_provider = legacy_store
            .get("userProvider")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let expires_at = legacy_store
            .get("expiresAt")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let agent_url = legacy_store
            .get("agentUrl")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();

        return Ok(Some(StoredSession {
            user_id,
            user_name,
            user_email,
            user_avatar_url,
            user_provider,
            token,
            expires_at,
            agent_url,
        }));
    }

    Ok(None)
}
