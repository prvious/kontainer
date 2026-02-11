use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeTaskItem {
    pub id: String,
    pub service_id: String,
    pub service_name: String,
    pub slot: i32,
    pub state: String,
    pub desired_state: String,
    pub error: String,
    pub timestamp: String,
    pub container_id: String,
}

#[derive(Debug, Deserialize)]
struct ListNodeTasksResponse {
    tasks: Vec<NodeTaskItem>,
}

#[tauri::command]
pub async fn list_node_tasks(
    url: String,
    token: String,
    node_id: String,
) -> Result<Vec<NodeTaskItem>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/api/nodes/{node_id}/tasks"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Agent responded with status {status}: {body}"));
    }

    let body: ListNodeTasksResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body.tasks)
}
