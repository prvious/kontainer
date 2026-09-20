use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub id: String,
    pub name: String,
    pub email: String,
    pub avatar_url: String,
    pub provider: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckAuthResponse {
    pub user: AuthUser,
    pub status: String,
}

#[tauri::command]
pub async fn check_auth(url: String, token: String) -> Result<CheckAuthResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/api/me"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    let status = response.status().as_u16();
    if status == 401 || status == 403 {
        return Err("auth_failed".to_string());
    }

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: CheckAuthResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
