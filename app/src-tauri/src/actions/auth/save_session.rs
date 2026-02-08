use tauri_plugin_store::StoreExt;

use super::{ConnectionProfile, SaveSessionInput};

#[tauri::command]
pub async fn save_session(app: tauri::AppHandle, input: SaveSessionInput) -> Result<(), String> {
    let store = app
        .store("profiles.json")
        .map_err(|e| format!("store error: {e}"))?;

    let mut profiles: Vec<ConnectionProfile> = store
        .get("profiles")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    // Find existing profile for this agent URL, or create new
    let profile_id = profiles
        .iter()
        .find(|p| p.agent_url == input.agent_url)
        .map(|p| p.id.clone())
        .unwrap_or_else(|| {
            format!("{:016x}", {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                input.user_id.hash(&mut hasher);
                input.agent_url.hash(&mut hasher);
                hasher.finish()
            })
        });

    let profile = ConnectionProfile {
        id: profile_id.clone(),
        label: "Default".to_string(),
        agent_url: input.agent_url,
        user_id: input.user_id,
        user_name: input.user_name,
        user_email: input.user_email,
        user_avatar_url: input.user_avatar_url,
        user_provider: input.user_provider,
        token: input.token,
        expires_at: input.expires_at,
    };

    if let Some(pos) = profiles.iter().position(|p| p.id == profile_id) {
        profiles[pos] = profile;
    } else {
        profiles.push(profile);
    }

    store.set(
        "profiles",
        serde_json::to_value(&profiles).map_err(|e| format!("serialize error: {e}"))?,
    );
    store.set("activeProfileId", serde_json::json!(profile_id));
    store.save().map_err(|e| format!("store save error: {e}"))?;

    Ok(())
}
