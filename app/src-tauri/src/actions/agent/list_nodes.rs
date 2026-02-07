use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeItem {
    pub id: String,
    pub hostname: String,
    pub role: String,
    pub status: String,
    pub availability: String,
    pub engine_version: String,
    pub os: String,
    pub architecture: String,
    pub ip: String,
    pub leader: bool,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListNodesResponse {
    pub nodes: Vec<NodeItem>,
}

#[tauri::command]
pub async fn list_nodes(url: String, token: String) -> Result<ListNodesResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/api/nodes"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: ListNodesResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
