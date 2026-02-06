pub mod delete_session;
pub mod load_session;
pub mod parse_auth_token;
pub mod save_session;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JwtPayload {
    pub user_id: String,
    pub name: String,
    pub email: String,
    pub avatar_url: String,
    pub provider: String,
    pub exp: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthUser {
    pub id: String,
    pub name: String,
    pub email: String,
    pub avatar_url: String,
    pub provider: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParsedToken {
    pub user: AuthUser,
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
