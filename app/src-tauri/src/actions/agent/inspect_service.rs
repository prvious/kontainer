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
    #[serde(rename = "type")]
    pub mount_type: String,
    pub source: String,
    pub target: String,
    pub read_only: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortInfo {
    pub published_port: u32,
    pub target_port: u32,
    pub protocol: String,
    pub publish_mode: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatusInfo {
    pub state: String,
    pub started_at: String,
    pub completed_at: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDetail {
    pub id: String,
    pub name: String,
    pub image: String,
    pub stack: String,
    pub mode: String,
    pub replicas: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
    pub env: Vec<EnvVar>,
    pub mounts: Vec<MountInfo>,
    pub constraints: Vec<String>,
    pub ports: Vec<PortInfo>,
    pub update_status: Option<UpdateStatusInfo>,
}

#[tauri::command]
pub async fn inspect_service(
    url: String,
    token: String,
    service_id: String,
) -> Result<ServiceDetail, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/api/services/{service_id}"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: ServiceDetail = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
