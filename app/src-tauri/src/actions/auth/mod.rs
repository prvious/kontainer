pub mod parse_auth_token;

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
