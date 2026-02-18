use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct SetupStatus {
    pub step: String,
}

#[tauri::command]
pub async fn setup_verify_token(url: String, token: String) -> Result<SetupStatus, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/setup/status"))
        .header("X-Kontainer-Setup-Token", &token)
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Invalid setup token".to_string());
    }

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: SetupStatus = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
