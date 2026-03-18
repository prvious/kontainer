use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MountInfo {
    pub r#type: String,
    pub source: String,
    pub target: String,
    pub read_only: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskNetworkInfo {
    pub name: String,
    pub addresses: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDetail {
    pub id: String,
    pub service_id: String,
    pub service_name: String,
    pub slot: i32,
    pub node_id: String,
    pub node_name: String,
    pub state: String,
    pub desired_state: String,
    pub timestamp: String,
    pub error: String,
    pub image: String,
    pub container_id: String,
    pub pid: i32,
    pub exit_code: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
    pub env: Vec<EnvVar>,
    pub mounts: Vec<MountInfo>,
    pub networks: Vec<TaskNetworkInfo>,
}

#[tauri::command]
pub async fn inspect_task(
    url: String,
    token: String,
    task_id: String,
) -> Result<TaskDetail, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/api/tasks/{task_id}"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: TaskDetail = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
