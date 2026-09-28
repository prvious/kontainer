use tauri::State;

use crate::session::errors::SessionError;
use crate::session::manager::SessionManager;
use crate::session::models::ProbeView;

/// Probe `/health/ready` at `origin` within two seconds and name the failed
/// step (`dns`, `tcp`, `tls`, `http`, `timeout`) when it is unreachable.
#[tauri::command]
pub async fn connection_probe(
    state: State<'_, SessionManager>,
    origin: String,
) -> Result<ProbeView, SessionError> {
    state.probe(&origin).await
}
