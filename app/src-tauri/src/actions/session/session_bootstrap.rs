use tauri::State;

use crate::session::errors::LoginFailure;
use crate::session::manager::SessionManager;
use crate::session::models::BootstrapView;

/// Whether the controller at `origin` already has its owner, which picks the
/// sign-in or create-owner form.
#[tauri::command]
pub async fn session_bootstrap(
    state: State<'_, SessionManager>,
    origin: String,
) -> Result<BootstrapView, LoginFailure> {
    state.bootstrap(&origin).await
}
