use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceItem {
    pub id: String,
    pub name: String,
    pub image: String,
    pub stack: String,
    pub replicas_running: i32,
    pub replicas_desired: i32,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListServicesResponse {
    pub services: Vec<ServiceItem>,
}

#[tauri::command]
pub async fn list_services(url: String, token: String) -> Result<ListServicesResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/api/services"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: ListServicesResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
