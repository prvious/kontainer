use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct ProvidersResponse {
    providers: Vec<String>,
}

#[tauri::command]
pub async fn fetch_providers(url: String) -> Result<Vec<String>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/auth/providers"))
        .send()
        .await
        .map_err(|e| format!("Could not fetch providers from {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Agent responded with status {}",
            response.status()
        ));
    }

    let body: ProvidersResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    Ok(body.providers)
}
