//! Session domain types: policy, states, and validated views.
//!
//! Persistence models stay in `entities`; HTTP DTOs stay in controllers.
//! This module owns the policy constants and the typed outcomes actions pass
//! to controllers.

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

/// Five-minute access credentials.
pub const ACCESS_TTL: Duration = Duration::minutes(5);
/// Thirty-day absolute session/refresh lifetime.
pub const ABSOLUTE_TTL: Duration = Duration::days(30);

#[must_use]
pub fn access_expiry(now: DateTime<Utc>) -> DateTime<Utc> {
    now + ACCESS_TTL
}

#[must_use]
pub fn absolute_expiry(now: DateTime<Utc>) -> DateTime<Utc> {
    now + ABSOLUTE_TTL
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalStatus {
    Pending,
    Active,
    Suspended,
    Deleted,
}

impl PrincipalStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Deleted => "deleted",
        }
    }
}

impl TryFrom<&str> for PrincipalStatus {
    type Error = CorruptSessionState;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "pending" => Ok(Self::Pending),
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            "deleted" => Ok(Self::Deleted),
            other => Err(CorruptSessionState(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevocationReason {
    Logout,
    RefreshReuse,
    PasswordReset,
}

impl RevocationReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Logout => "logout",
            Self::RefreshReuse => "refresh_reuse",
            Self::PasswordReset => "password_reset",
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("session persistence holds an unknown state")]
pub struct CorruptSessionState(String);

/// Session proven against live server-side state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedSession {
    pub principal_id: Uuid,
    pub family_id: Uuid,
    /// When the presented access credential stops being valid; long-lived streams close then.
    pub access_expires_at: DateTime<Utc>,
}

/// Authenticated principal derived from live persisted session state.
///
/// Handlers receive this type; they never parse credentials or trust
/// principal claims from request input. The inner UUID is the immutable
/// persisted principal ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrincipalContext {
    principal_id: Uuid,
}

impl PrincipalContext {
    #[must_use]
    pub fn new(principal_id: Uuid) -> Self {
        Self { principal_id }
    }

    #[must_use]
    pub fn principal_id(&self) -> Uuid {
        self.principal_id
    }

    /// Canonical string stored in operation `principal_ref` columns.
    #[must_use]
    pub fn reference(&self) -> String {
        self.principal_id.to_string()
    }
}

impl From<ValidatedSession> for PrincipalContext {
    fn from(session: ValidatedSession) -> Self {
        Self::new(session.principal_id)
    }
}

/// Credentials issued exactly once, alongside their expirations.
pub struct IssuedSession {
    pub principal_id: Uuid,
    pub family_id: Uuid,
    pub access_plaintext: String,
    pub refresh_plaintext: String,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
}

impl std::fmt::Debug for IssuedSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IssuedSession")
            .field("principal_id", &self.principal_id)
            .field("family_id", &self.family_id)
            .field("access_plaintext", &"<redacted>")
            .field("refresh_plaintext", &"<redacted>")
            .field("access_expires_at", &self.access_expires_at)
            .field("refresh_expires_at", &self.refresh_expires_at)
            .finish()
    }
}

pub mod actions {
    pub const CREATE: &str = "session.create";
    pub const ROTATE: &str = "session.refresh.rotate";
    pub const REUSE: &str = "session.refresh.reuse";
    pub const REVOKE: &str = "session.revoke";
    pub const PENDING: &str = "pending";
}

#[cfg(test)]
mod tests {
    use super::{PrincipalContext, PrincipalStatus, RevocationReason};

    #[test]
    fn principal_statuses_have_stable_values() {
        for (value, status) in [
            ("pending", PrincipalStatus::Pending),
            ("active", PrincipalStatus::Active),
            ("suspended", PrincipalStatus::Suspended),
            ("deleted", PrincipalStatus::Deleted),
        ] {
            assert_eq!(PrincipalStatus::try_from(value).unwrap(), status);
            assert_eq!(status.as_str(), value);
        }
    }

    #[test]
    fn revocation_reasons_are_stable() {
        assert_eq!(RevocationReason::Logout.as_str(), "logout");
        assert_eq!(RevocationReason::RefreshReuse.as_str(), "refresh_reuse");
        assert_eq!(RevocationReason::PasswordReset.as_str(), "password_reset");
    }

    #[test]
    fn principal_context_exposes_only_the_persisted_id() {
        let id = uuid::Uuid::now_v7();
        let context = PrincipalContext::new(id);

        assert_eq!(context.principal_id(), id);
        assert_eq!(context.reference(), id.to_string());
        assert_eq!(
            format!("{context:?}"),
            format!("PrincipalContext {{ principal_id: {id} }}")
        );
    }

    #[test]
    fn issued_session_debug_is_redacted() {
        let issued = super::IssuedSession {
            principal_id: uuid::Uuid::now_v7(),
            family_id: uuid::Uuid::now_v7(),
            access_plaintext: "kta_secret-canary".to_owned(),
            refresh_plaintext: "ktr_secret-canary".to_owned(),
            access_expires_at: chrono::Utc::now(),
            refresh_expires_at: chrono::Utc::now(),
        };
        let debug = format!("{issued:?}");
        assert!(!debug.contains("secret-canary"));
    }
}
