use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackItem {
    pub name: String,
    pub services: i32,
    pub running_replicas: i32,
    pub desired_replicas: i32,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListStacksResponse {
    pub stacks: Vec<StackItem>,
}

#[tauri::command]
pub async fn list_stacks(url: String, token: String) -> Result<ListStacksResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/api/stacks"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: ListStacksResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
