pub mod auth_listen;
pub mod delete_profile;
pub mod delete_session;
pub mod load_profiles;
pub mod load_session;
pub mod parse_auth_token;
pub mod save_profile;
pub mod save_session;
pub mod set_active_profile;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JwtPayload {
    pub key: String,
    pub exp: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParsedToken {
    pub user_id: String,
    pub expires_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveSessionInput {
    pub user_id: String,
    pub user_name: String,
    pub user_email: String,
    pub user_avatar_url: String,
    pub user_provider: String,
    pub token: String,
    pub expires_at: u64,
    pub agent_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredSession {
    pub user_id: String,
    pub user_name: String,
    pub user_email: String,
    pub user_avatar_url: String,
    pub user_provider: String,
    pub token: String,
    pub expires_at: u64,
    pub agent_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConnectionProfile {
    pub id: String,
    pub label: String,
    pub agent_url: String,
    pub user_id: String,
    pub user_name: String,
    pub user_email: String,
    pub user_avatar_url: String,
    pub user_provider: String,
    pub token: String,
    pub expires_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredProfiles {
    pub profiles: Vec<ConnectionProfile>,
    pub active_profile_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveProfileInput {
    pub id: String,
    pub label: String,
    pub agent_url: String,
    pub user_id: String,
    pub user_name: String,
    pub user_email: String,
    pub user_avatar_url: String,
    pub user_provider: String,
    pub token: String,
    pub expires_at: u64,
}
