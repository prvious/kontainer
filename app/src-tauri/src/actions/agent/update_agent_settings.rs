use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateAgentSettingsResponse {
    pub settings: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct UpdateBody {
    settings: HashMap<String, String>,
}

#[tauri::command]
pub async fn update_agent_settings(
    url: String,
    token: String,
    settings: HashMap<String, String>,
) -> Result<UpdateAgentSettingsResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .put(format!("{url}/api/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&UpdateBody { settings })
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: UpdateAgentSettingsResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
