use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNodeRoleResponse {
    pub role: String,
}

#[derive(Debug, Serialize)]
struct UpdateRoleBody {
    role: String,
}

#[tauri::command]
pub async fn update_node_role(
    url: String,
    token: String,
    node_id: String,
    role: String,
) -> Result<UpdateNodeRoleResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .put(format!("{url}/api/nodes/{node_id}/role"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&UpdateRoleBody { role })
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Agent responded with status {status}: {body}"));
    }

    let body: UpdateNodeRoleResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
