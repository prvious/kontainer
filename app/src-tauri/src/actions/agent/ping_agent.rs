use std::collections::HashMap;
use std::time::Duration;

#[tauri::command]
pub async fn ping_agent(url: String) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(format!("{url}/ping"))
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Agent responded with status {}", response.status()));
    }

    let body: HashMap<String, String> = response
        .json()
        .await
        .map_err(|e| format!("Invalid response from agent: {e}"))?;

    match body.get("app") {
        Some(app) if app == "dev.devoper.app" => Ok(()),
        _ => Err("Not a valid devoper agent".to_string()),
    }
}
