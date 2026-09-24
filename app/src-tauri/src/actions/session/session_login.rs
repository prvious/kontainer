use tauri::State;

use crate::session::errors::SessionError;
use crate::session::manager::SessionManager;
use crate::session::models::{SessionLoginInput, SessionView};

/// Start a browser login against a validated controller origin.
///
/// An earlier attempt is cancelled and joined before this one generates its
/// verifier. The authorization URL, OAuth state, verifier, and credentials
/// never cross this boundary.
#[tauri::command]
pub async fn session_login(
    state: State<'_, SessionManager>,
    origin: String,
    label: Option<String>,
) -> Result<SessionView, SessionError> {
    state.login(SessionLoginInput { origin, label }).await
}
