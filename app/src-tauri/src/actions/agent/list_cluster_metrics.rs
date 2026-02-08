use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeMetrics {
    pub cpus: i32,
    pub memory_total: u64,
    pub memory_used: u64,
    pub memory_percent: f64,
    pub cpu_percent: f64,
    pub disk_total: u64,
    pub disk_used: u64,
    pub disk_percent: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerMetrics {
    pub container_id: String,
    pub name: String,
    pub service_name: String,
    pub cpu_percent: f64,
    pub memory_used: u64,
    pub memory_limit: u64,
    pub memory_percent: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMetricsSnapshot {
    pub timestamp: String,
    pub node_id: String,
    pub hostname: String,
    pub node: NodeMetrics,
    pub containers: Vec<ContainerMetrics>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterMetricsResponse {
    pub agents: Vec<AgentMetricsSnapshot>,
    pub history: Vec<AgentMetricsSnapshot>,
}

#[tauri::command]
pub async fn list_cluster_metrics(
    url: String,
    token: String,
) -> Result<ClusterMetricsResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/api/metrics"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: ClusterMetricsResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
