use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

use super::{JwtPayload, ParsedToken};

#[tauri::command]
pub fn parse_auth_token(token: &str) -> Result<ParsedToken, String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err("invalid token format".into());
    }

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|e| format!("base64 decode error: {e}"))?;

    let payload: JwtPayload =
        serde_json::from_slice(&payload_bytes).map_err(|e| format!("json parse error: {e}"))?;

    Ok(ParsedToken {
        user_id: payload.key,
        expires_at: payload.exp,
    })
}
