use tauri_plugin_store::StoreExt;

use super::{ConnectionProfile, SaveProfileInput};

#[tauri::command]
pub async fn save_profile(app: tauri::AppHandle, input: SaveProfileInput) -> Result<(), String> {
    let store = app
        .store("profiles.json")
        .map_err(|e| format!("store error: {e}"))?;

    let mut profiles: Vec<ConnectionProfile> = store
        .get("profiles")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let profile = ConnectionProfile {
        id: input.id.clone(),
        label: input.label,
        agent_url: input.agent_url,
        user_id: input.user_id,
        user_name: input.user_name,
        user_email: input.user_email,
        user_avatar_url: input.user_avatar_url,
        user_provider: input.user_provider,
        token: input.token,
        expires_at: input.expires_at,
    };

    // Upsert: replace existing or append
    if let Some(pos) = profiles.iter().position(|p| p.id == input.id) {
        profiles[pos] = profile;
    } else {
        profiles.push(profile);
    }

    store.set(
        "profiles",
        serde_json::to_value(&profiles).map_err(|e| format!("serialize error: {e}"))?,
    );
    store.set("activeProfileId", serde_json::json!(input.id));
    store.save().map_err(|e| format!("store save error: {e}"))?;

    Ok(())
}
