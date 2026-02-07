use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaleServiceResponse {
    pub replicas: u64,
}

#[derive(Debug, Serialize)]
struct ScaleServiceBody {
    replicas: u64,
}

#[tauri::command]
pub async fn scale_service(
    url: String,
    token: String,
    service_id: String,
    replicas: u64,
) -> Result<ScaleServiceResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .patch(format!("{url}/api/services/{service_id}"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&ScaleServiceBody { replicas })
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Agent responded with status {status}: {body}"));
    }

    let body: ScaleServiceResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
