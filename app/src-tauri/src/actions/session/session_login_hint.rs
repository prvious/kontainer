use tauri::State;

use crate::session::manager::SessionManager;
use crate::session::models::LoginHint;

/// The non-secret origin/label hint for the login screen, when one exists.
///
/// This never fails: a metadata store that cannot be read simply yields no
/// hint, and no session data is exposed through this path.
#[tauri::command]
pub fn session_login_hint(state: State<'_, SessionManager>) -> Option<LoginHint> {
    state.login_hint()
}
