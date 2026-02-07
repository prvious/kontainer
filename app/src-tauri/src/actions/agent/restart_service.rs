use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct RestartServiceResponse {
    pub status: String,
}

#[tauri::command]
pub async fn restart_service(
    url: String,
    token: String,
    service_id: String,
) -> Result<RestartServiceResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .post(format!("{url}/api/services/{service_id}/restart"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Agent responded with status {status}: {body}"));
    }

    let body: RestartServiceResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
