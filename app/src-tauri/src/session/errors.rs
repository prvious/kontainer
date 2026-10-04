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
    /// Email or password is incorrect; `LoginFailure` carries attempts left.
    InvalidCredentials,
    /// Sign-in is locked for this email; `LoginFailure` carries the wait.
    LoginLocked,
    /// Registration is closed because the controller already has its owner.
    OwnerExists,
    /// Submitted account fields were rejected; `LoginFailure` names them.
    ValidationFailed,
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
        SessionError::InvalidCredentials,
        SessionError::LoginLocked,
        SessionError::OwnerExists,
        SessionError::ValidationFailed,
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
        | SessionError::InvalidCredentials
        | SessionError::LoginLocked
        | SessionError::OwnerExists
        | SessionError::ValidationFailed
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

/// Why the controller rejected one submitted account field.
pub(crate) use kontainer_native_client::FieldProblem;

/// Field problems for the sign-in and create-owner forms.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FieldErrorsView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<FieldProblem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<FieldProblem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<FieldProblem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup_code: Option<FieldProblem>,
}

/// A failed sign-in, registration, or bootstrap check: the closed code plus
/// the details the connect card shows. Never carries a credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoginFailure {
    pub code: SessionError,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempts_remaining: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldErrorsView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl From<SessionError> for LoginFailure {
    fn from(code: SessionError) -> Self {
        Self {
            code,
            attempts_remaining: None,
            retry_after_seconds: None,
            fields: None,
            request_id: None,
        }
    }
}

/// Lets tests compare a failure with the bare code it carries.
impl PartialEq<SessionError> for LoginFailure {
    fn eq(&self, other: &SessionError) -> bool {
        self.code == *other
    }
}
