use tauri::State;
use zeroize::Zeroizing;

use crate::session::errors::LoginFailure;
use crate::session::manager::SessionManager;
use crate::session::models::{RegisterOwnerInput, SessionView};

/// Create the owner on a controller that has none yet, then sign in.
/// The password and the setup code the controller printed at startup are
/// handled exactly as the password is in [`super::session_login`].
#[tauri::command]
pub async fn session_register(
    state: State<'_, SessionManager>,
    origin: String,
    display_name: String,
    email: String,
    password: String,
    setup_code: String,
    label: Option<String>,
) -> Result<SessionView, LoginFailure> {
    state
        .register(RegisterOwnerInput {
            origin,
            display_name,
            email,
            password: Zeroizing::new(password),
            setup_code: Zeroizing::new(setup_code),
            label,
        })
        .await
}
