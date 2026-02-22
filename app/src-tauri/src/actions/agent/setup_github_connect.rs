use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestResponse {
    pub setup_url: String,
}

#[tauri::command]
pub async fn setup_github_connect(
    url: String,
    token: String,
    org: Option<String>,
) -> Result<ManifestResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let mut request = client
        .post(format!("{url}/setup/github/manifest"))
        .header("X-Kontainer-Setup-Token", &token);

    if let Some(ref org) = org {
        request = request.json(&serde_json::json!({ "org": org }));
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: ManifestResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
