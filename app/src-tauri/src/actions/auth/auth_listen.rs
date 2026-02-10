use futures_util::StreamExt;
use serde::Deserialize;
use tauri::Emitter;
use tokio_tungstenite::connect_async;

#[derive(Deserialize)]
struct WsMessage {
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Clone, serde::Serialize)]
struct SessionIdPayload {
    session_id: String,
}

#[tauri::command]
pub async fn auth_listen(app: tauri::AppHandle, url: String) -> Result<String, String> {
    let ws_url = url.replace("http", "ws") + "/auth/listen";

    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .map_err(|e| format!("WebSocket connection failed: {e}"))?;

    let (_, mut read) = ws_stream.split();

    while let Some(msg) = read.next().await {
        let msg = msg.map_err(|e| format!("WebSocket read error: {e}"))?;

        let text = match msg {
            tokio_tungstenite::tungstenite::Message::Text(t) => t,
            _ => continue,
        };

        let parsed: WsMessage =
            serde_json::from_str(&text).map_err(|e| format!("Invalid message: {e}"))?;

        if let Some(session_id) = parsed.session_id {
            app.emit("auth://session-id", SessionIdPayload { session_id })
                .map_err(|e| format!("Failed to emit event: {e}"))?;
            continue;
        }

        if let Some(token) = parsed.token {
            return Ok(token);
        }

        if let Some(error) = parsed.error {
            return Err(error);
        }
    }

    Err("WebSocket closed without receiving token".to_string())
}
