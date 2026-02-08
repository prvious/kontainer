use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNodeAvailabilityResponse {
    pub availability: String,
}

#[derive(Debug, Serialize)]
struct UpdateAvailabilityBody {
    availability: String,
}

#[tauri::command]
pub async fn update_node_availability(
    url: String,
    token: String,
    node_id: String,
    availability: String,
) -> Result<UpdateNodeAvailabilityResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .put(format!("{url}/api/nodes/{node_id}/availability"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&UpdateAvailabilityBody { availability })
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Agent responded with status {status}: {body}"));
    }

    let body: UpdateNodeAvailabilityResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
