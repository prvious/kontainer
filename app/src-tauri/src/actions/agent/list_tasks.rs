use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub id: String,
    pub service_id: String,
    pub service_name: String,
    pub slot: i32,
    pub node_id: String,
    pub node_name: String,
    pub state: String,
    pub desired_state: String,
    pub exit_code: Option<i32>,
    pub error: String,
    pub timestamp: String,
    pub container_id: String,
    pub image: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListTasksResponse {
    pub tasks: Vec<TaskItem>,
}

#[tauri::command]
pub async fn list_tasks(url: String, token: String) -> Result<ListTasksResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/api/tasks"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: ListTasksResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
