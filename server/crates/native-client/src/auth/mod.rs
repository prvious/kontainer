//! Password sign-in and owner registration, GitHub bootstrap login, refresh,
//! and logout contracts.
//!
//! The controller issues credentials exactly once, in a `no-store` JSON
//! response. The client never retries these calls: a replayed login,
//! redemption, or refresh would be a credential-safety incident, not a
//! transient failure.

mod bootstrap_status;
mod current_session;
mod logout;
mod password_login;
mod password_session;
mod redeem_login;
mod refresh_session;
mod register_owner;
mod start_login;

#[cfg(test)]
pub(crate) use bootstrap_status::BootstrapReply;
pub use current_session::Account;
#[cfg(test)]
pub(crate) use current_session::AccountReply;
#[cfg(test)]
pub(crate) use password_login::PasswordLoginBody;
pub use password_session::PasswordSession;
#[cfg(test)]
pub(crate) use password_session::PasswordSessionReply;
#[cfg(test)]
pub(crate) use register_owner::RegisterOwnerBody;

#[cfg(test)]
pub(crate) use redeem_login::{RedeemLoginBody, RedeemLoginReply};
#[cfg(test)]
pub(crate) use refresh_session::{RefreshSessionBody, RefreshSessionReply};
#[cfg(test)]
pub(crate) use start_login::{StartLoginBody, StartLoginReply};

use chrono::{DateTime, Utc};
use url::Url;
use uuid::Uuid;

use crate::credentials::{AccessCredential, RefreshCredential};

/// A started GitHub login attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginAttempt {
    /// Single-use attempt identifier the desktop redeems after the browser
    /// step completes.
    pub attempt_id: Uuid,
    /// GitHub authorization URL to open in the system browser.
    pub authorize_url: Url,
    /// Attempt expiry reported by the controller.
    pub expires_at: DateTime<Utc>,
}

/// The credential pair issued by redemption or refresh.
///
/// Credentials are returned to the caller exactly once; the client never
/// stores, retries, or logs them.
#[derive(Debug)]
pub struct SessionCredentials {
    /// Short-lived access credential for `Authorization: Bearer` requests.
    pub access: AccessCredential,
    /// Rotating refresh credential. Presenting a consumed one revokes the
    /// whole session family.
    pub refresh: RefreshCredential,
    /// Access-credential expiry reported by the controller.
    pub access_expires_at: DateTime<Utc>,
    /// Refresh-credential expiry reported by the controller.
    pub refresh_expires_at: DateTime<Utc>,
}

/// A redeemed bootstrap login: the authenticated principal plus its session.
#[derive(Debug)]
pub struct AuthenticatedSession {
    /// Controller cluster identity this session belongs to.
    pub cluster_id: String,
    /// Immutable persisted principal identifier.
    pub principal_id: Uuid,
    /// Mutable GitHub display login.
    pub github_login: String,
    /// Freshly issued credentials.
    pub credentials: SessionCredentials,
}
