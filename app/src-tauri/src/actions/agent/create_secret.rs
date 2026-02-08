use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize)]
struct CreateSecretBody {
    name: String,
    data: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSecretResponse {
    pub id: String,
    pub name: String,
}

#[tauri::command]
pub async fn create_secret(
    url: String,
    token: String,
    name: String,
    data: String,
) -> Result<CreateSecretResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .post(format!("{url}/api/secrets"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&CreateSecretBody { name, data })
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Agent responded with status {status}: {body}"));
    }

    let body: CreateSecretResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body)
}
