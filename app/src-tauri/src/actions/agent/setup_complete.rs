use std::time::Duration;

#[tauri::command]
pub async fn setup_complete(url: String, token: String) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .post(format!("{url}/setup/complete"))
        .header("X-Kontainer-Setup-Token", &token)
        .send()
        .await
        .map_err(|e| format!("Could not reach agent at {url}: {e}"))?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Setup completion failed: {body}"));
    }

    Ok(())
}
