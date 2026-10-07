use tauri::State;
use zeroize::Zeroizing;

use crate::session::errors::LoginFailure;
use crate::session::manager::SessionManager;
use crate::session::models::{PasswordLoginInput, SessionView};

/// Sign in to a controller with email and password.
///
/// The password exists only as this request argument: it is zeroed after the
/// sign-in call and never stored, logged, or returned. Credentials stay in
/// Rust custody; React receives only the credential-free [`SessionView`].
#[tauri::command]
pub async fn session_login(
    state: State<'_, SessionManager>,
    origin: String,
    email: String,
    password: String,
    label: Option<String>,
) -> Result<SessionView, LoginFailure> {
    state
        .login(PasswordLoginInput {
            origin,
            email,
            password: Zeroizing::new(password),
            label,
        })
        .await
}
