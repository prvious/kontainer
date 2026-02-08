use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeItem {
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub mountpoint: String,
    pub labels: HashMap<String, String>,
    pub options: HashMap<String, String>,
    pub created_at: String,
    pub ref_count: i64,
    pub size_bytes: i64,
    pub stack: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListVolumesResponse {
    pub volumes: Vec<VolumeItem>,
}

#[tauri::command]
pub async fn list_volumes(url: String, token: String) -> Result<ListVolumesResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/api/volumes"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: ListVolumesResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
