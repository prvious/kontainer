use tauri::State;
use zeroize::Zeroizing;

use crate::session::errors::LoginFailure;
use crate::session::manager::SessionManager;
use crate::session::models::{RegisterOwnerInput, SessionView};

/// Create the owner on a controller that has none yet, then sign in.
/// The password is handled exactly as in [`super::session_login`].
#[tauri::command]
pub async fn session_register(
    state: State<'_, SessionManager>,
    origin: String,
    display_name: String,
    email: String,
    password: String,
    label: Option<String>,
) -> Result<SessionView, LoginFailure> {
    state
        .register(RegisterOwnerInput {
            origin,
            display_name,
            email,
            password: Zeroizing::new(password),
            label,
        })
        .await
}
