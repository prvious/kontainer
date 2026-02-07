use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceTaskItem {
    pub id: String,
    pub slot: i32,
    pub node_id: String,
    pub node_name: String,
    pub state: String,
    pub desired_state: String,
    pub exit_code: Option<i32>,
    pub error: String,
    pub timestamp: String,
    pub container_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListServiceTasksResponse {
    pub tasks: Vec<ServiceTaskItem>,
}

#[tauri::command]
pub async fn list_service_tasks(
    url: String,
    token: String,
    service_id: String,
) -> Result<ListServiceTasksResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/api/services/{service_id}/tasks"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: ListServiceTasksResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
