use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveStackResponse {
    pub removed: i32,
}

#[tauri::command]
pub async fn remove_stack(
    url: String,
    token: String,
    stack_name: String,
) -> Result<RemoveStackResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .delete(format!("{url}/api/stacks/{stack_name}"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Agent responded with status {status}: {body}"));
    }

    let body: RemoveStackResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
