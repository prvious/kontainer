use tauri_plugin_store::StoreExt;

use super::{ConnectionProfile, StoredProfiles, StoredSession};

#[tauri::command]
pub async fn load_profiles(app: tauri::AppHandle) -> Result<StoredProfiles, String> {
    let store = app
        .store("profiles.json")
        .map_err(|e| format!("store error: {e}"))?;

    let profiles_val = store.get("profiles");
    let active_id_val = store.get("activeProfileId");

    if let Some(profiles_json) = profiles_val {
        let profiles: Vec<ConnectionProfile> =
            serde_json::from_value(profiles_json).unwrap_or_default();
        let active_profile_id = active_id_val.and_then(|v| v.as_str().map(String::from));

        return Ok(StoredProfiles {
            profiles,
            active_profile_id,
        });
    }

    // Migration: try reading from legacy session.json
    if let Ok(legacy_store) = app.store("session.json") {
        let user_id = legacy_store
            .get("userId")
            .and_then(|v| v.as_str().map(String::from));

        if let Some(user_id) = user_id {
            if !user_id.is_empty() {
                let token = legacy_store
                    .get("token")
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default();

                if !token.is_empty() {
                    let session = StoredSession {
                        user_id: user_id.clone(),
                        user_name: legacy_store
                            .get("userName")
                            .and_then(|v| v.as_str().map(String::from))
                            .unwrap_or_default(),
                        user_email: legacy_store
                            .get("userEmail")
                            .and_then(|v| v.as_str().map(String::from))
                            .unwrap_or_default(),
                        user_avatar_url: legacy_store
                            .get("userAvatarUrl")
                            .and_then(|v| v.as_str().map(String::from))
                            .unwrap_or_default(),
                        user_provider: legacy_store
                            .get("userProvider")
                            .and_then(|v| v.as_str().map(String::from))
                            .unwrap_or_default(),
                        token: token.clone(),
                        expires_at: legacy_store
                            .get("expiresAt")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0),
                        agent_url: legacy_store
                            .get("agentUrl")
                            .and_then(|v| v.as_str().map(String::from))
                            .unwrap_or_default(),
                    };

                    let profile_id = uuid_from_session(&session);
                    let profile = ConnectionProfile {
                        id: profile_id.clone(),
                        label: "Default".to_string(),
                        agent_url: session.agent_url,
                        user_id: session.user_id,
                        user_name: session.user_name,
                        user_email: session.user_email,
                        user_avatar_url: session.user_avatar_url,
                        user_provider: session.user_provider,
                        token: session.token,
                        expires_at: session.expires_at,
                    };

                    let stored = StoredProfiles {
                        profiles: vec![profile],
                        active_profile_id: Some(profile_id),
                    };

                    // Save migrated data to profiles.json
                    store.set(
                        "profiles",
                        serde_json::to_value(&stored.profiles)
                            .map_err(|e| format!("serialize error: {e}"))?,
                    );
                    store.set(
                        "activeProfileId",
                        serde_json::json!(stored.active_profile_id),
                    );
                    store.save().map_err(|e| format!("store save error: {e}"))?;

                    // Clear legacy store
                    legacy_store.clear();
                    let _ = legacy_store.save();

                    return Ok(stored);
                }
            }
        }
    }

    Ok(StoredProfiles {
        profiles: vec![],
        active_profile_id: None,
    })
}

fn uuid_from_session(session: &StoredSession) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    session.user_id.hash(&mut hasher);
    session.agent_url.hash(&mut hasher);
    let hash = hasher.finish();
    format!("{hash:016x}")
}
