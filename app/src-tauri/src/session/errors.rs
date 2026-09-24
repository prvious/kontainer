//! Closed session error codes crossing the desktop IPC boundary.
//!
//! Every failure the custody layer can report maps to exactly one of these
//! codes. Raw transport, vault, controller, or metadata text never crosses the
//! Tauri boundary.

use serde::Serialize;

/// Every failure a session custody operation can report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SessionError {
    /// The supplied controller origin failed validation.
    InvalidOrigin,
    /// The system browser could not be opened for the authorization URL.
    BrowserOpenFailed,
    /// The operator cancelled the active login attempt.
    LoginCancelled,
    /// The login attempt expired before the browser step completed.
    LoginExpired,
    /// The controller denied the login (allowlist, replay, or expiry).
    LoginDenied,
    /// The identity provider could not be reached for the exchange.
    ProviderUnavailable,
    /// The controller could not be reached or its exchange did not complete.
    ControllerUnavailable,
    /// The controller response violated the documented contract.
    InvalidResponse,
    /// A session is active, or this profile is bound to another pair.
    ProfileConflict,
    /// The stored session is invalid or was invalidated; log in again.
    ///
    /// The custody layer reached this outcome only after local invalidation
    /// completed, so no stored credential is expected to remain.
    ReloginRequired,
    /// The OS credential vault is unavailable or locked.
    VaultUnavailable,
    /// The OS credential vault rejected a write.
    ///
    /// A freshly issued successor retained in memory can be committed with
    /// `retry_commit` without another controller call.
    VaultWriteFailed,
    /// The non-secret metadata store could not be read or saved, or no local
    /// commit exists to retry.
    LocalStateFailed,
    /// Server-side revocation failed; vault custody is preserved for retry.
    RevokeFailed,
    /// The server session is gone but local credential deletion failed.
    LocalCleanupFailed,
    /// Custody cannot prove the session is revoked or was never issued: an
    /// uncommitted rotation window, or a logout with no reachable credential.
    /// An explicit logout or commit retry must resolve it before login.
    CleanupUncertain,
}

#[cfg(test)]
impl SessionError {
    /// Every variant in declaration order; the shared IPC fixture and the
    /// TypeScript union are checked against this list.
    pub(crate) const ALL: &'static [SessionError] = &[
        SessionError::InvalidOrigin,
        SessionError::BrowserOpenFailed,
        SessionError::LoginCancelled,
        SessionError::LoginExpired,
        SessionError::LoginDenied,
        SessionError::ProviderUnavailable,
        SessionError::ControllerUnavailable,
        SessionError::InvalidResponse,
        SessionError::ProfileConflict,
        SessionError::ReloginRequired,
        SessionError::VaultUnavailable,
        SessionError::VaultWriteFailed,
        SessionError::LocalStateFailed,
        SessionError::RevokeFailed,
        SessionError::LocalCleanupFailed,
        SessionError::CleanupUncertain,
    ];
}

/// Compile-time exhaustiveness guard: adding a variant must break this match,
/// which in turn forces `ALL` and the shared IPC fixture to be updated.
#[cfg(test)]
#[allow(dead_code)]
fn session_error_is_exhaustive(error: SessionError) {
    match error {
        SessionError::InvalidOrigin
        | SessionError::BrowserOpenFailed
        | SessionError::LoginCancelled
        | SessionError::LoginExpired
        | SessionError::LoginDenied
        | SessionError::ProviderUnavailable
        | SessionError::ControllerUnavailable
        | SessionError::InvalidResponse
        | SessionError::ProfileConflict
        | SessionError::ReloginRequired
        | SessionError::VaultUnavailable
        | SessionError::VaultWriteFailed
        | SessionError::LocalStateFailed
        | SessionError::RevokeFailed
        | SessionError::LocalCleanupFailed
        | SessionError::CleanupUncertain => {}
    }
}
