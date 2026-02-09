use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveImageResponse {
    pub deleted: Vec<String>,
    pub untagged: Vec<String>,
}

#[tauri::command]
pub async fn remove_image(
    url: String,
    token: String,
    image_id: String,
    node_id: String,
    force: bool,
) -> Result<RemoveImageResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .delete(format!(
            "{url}/api/images/{image_id}?node={node_id}&force={force}"
        ))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Agent responded with status {status}: {body}"));
    }

    let body: RemoveImageResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
