use super::list_cluster_metrics::AgentMetricsSnapshot;
use std::time::Duration;

#[tauri::command]
pub async fn list_node_metrics(
    url: String,
    token: String,
    node_id: String,
) -> Result<AgentMetricsSnapshot, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/api/metrics/{node_id}"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: AgentMetricsSnapshot = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
