use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateServiceImageResponse {
    pub image: String,
}

#[derive(Debug, Serialize)]
struct UpdateImageBody {
    image: String,
}

#[tauri::command]
pub async fn update_service_image(
    url: String,
    token: String,
    service_id: String,
    image: String,
) -> Result<UpdateServiceImageResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .put(format!("{url}/api/services/{service_id}/image"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&UpdateImageBody { image })
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Agent responded with status {status}: {body}"));
    }

    let body: UpdateServiceImageResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
