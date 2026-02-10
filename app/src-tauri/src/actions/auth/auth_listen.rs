use futures_util::StreamExt;
use serde::Deserialize;
use tauri::Emitter;
use tokio_tungstenite::connect_async;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WsMessage {
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionIdPayload {
    session_id: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TokenPayload {
    token: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorPayload {
    error: String,
}

#[tauri::command]
pub async fn auth_listen(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let ws_url = if url.starts_with("https://") {
        url.replacen("https://", "wss://", 1)
    } else {
        url.replacen("http://", "ws://", 1)
    } + "/auth/listen";

    tauri::async_runtime::spawn(async move {
        if let Err(e) = connect_and_listen(&app, &ws_url).await {
            let _ = app.emit("auth://error", ErrorPayload { error: e });
        }
    });

    Ok(())
}

async fn connect_and_listen(app: &tauri::AppHandle, ws_url: &str) -> Result<(), String> {
    let (ws_stream, _) = connect_async(ws_url)
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
            app.emit("auth://token", TokenPayload { token })
                .map_err(|e| format!("Failed to emit event: {e}"))?;
            return Ok(());
        }

        if let Some(error) = parsed.error {
            return Err(error);
        }
    }

    Err("WebSocket closed without receiving token".to_string())
}
