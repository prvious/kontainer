use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::entities::operation;
use crate::auth::domain::PrincipalContext;

const MAX_CANONICAL_REQUEST_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationState {
    Recoverable,
    Running,
    Uncertain,
    Succeeded,
    Failed,
    Cancelled,
}

impl OperationState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Recoverable => "recoverable",
            Self::Running => "running",
            Self::Uncertain => "uncertain",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    #[must_use]
    pub const fn is_recoverable(self) -> bool {
        matches!(self, Self::Recoverable)
    }

    #[must_use]
    pub const fn is_uncertain(self) -> bool {
        matches!(self, Self::Uncertain)
    }
}

impl TryFrom<&str> for OperationState {
    type Error = CorruptOperationState;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "recoverable" => Ok(Self::Recoverable),
            "running" => Ok(Self::Running),
            "uncertain" => Ok(Self::Uncertain),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(CorruptOperationState(other.to_owned())),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("operation has an unknown persisted state")]
pub struct CorruptOperationState(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurableOperation {
    pub id: Uuid,
    pub kind: String,
    pub state: OperationState,
    pub claimed_by: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<operation::Model> for DurableOperation {
    type Error = CorruptOperationState;

    fn try_from(model: operation::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: model.id,
            kind: model.kind,
            state: OperationState::try_from(model.state.as_str())?,
            claimed_by: model.claimed_by,
            claimed_at: model.claimed_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationSnapshot {
    pub operation: DurableOperation,
    pub last_event_sequence: i32,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AdmitOperationRequest {
    principal: PrincipalContext,
    scope: String,
    idempotency_key: String,
    kind: String,
    audit_action: String,
    audit_target_ref: Option<String>,
    canonical_request: Vec<u8>,
    idempotency_expires_at: DateTime<Utc>,
}

impl AdmitOperationRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        principal: PrincipalContext,
        scope: impl Into<String>,
        idempotency_key: impl Into<String>,
        kind: impl Into<String>,
        audit_action: impl Into<String>,
        audit_target_ref: Option<String>,
        canonical_request: Vec<u8>,
        idempotency_expires_at: DateTime<Utc>,
    ) -> Result<Self, AdmissionInputError> {
        let scope = validated_text("idempotency scope", scope.into(), 128)?;
        let idempotency_key = validated_text("idempotency key", idempotency_key.into(), 512)?;
        let kind = validated_text("operation kind", kind.into(), 64)?;
        let audit_action = validated_text("audit action", audit_action.into(), 64)?;
        let audit_target_ref = audit_target_ref
            .map(|target| validated_text("audit target reference", target, 255))
            .transpose()?;

        if canonical_request.len() > MAX_CANONICAL_REQUEST_BYTES {
            return Err(AdmissionInputError::CanonicalRequestTooLarge);
        }
        if idempotency_expires_at <= Utc::now() {
            return Err(AdmissionInputError::ExpiredIdempotencyRecord);
        }

        Ok(Self {
            principal,
            scope,
            idempotency_key,
            kind,
            audit_action,
            audit_target_ref,
            canonical_request,
            idempotency_expires_at,
        })
    }

    #[must_use]
    pub fn principal(&self) -> &PrincipalContext {
        &self.principal
    }

    #[must_use]
    pub fn scope(&self) -> &str {
        &self.scope
    }

    #[must_use]
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn audit_action(&self) -> &str {
        &self.audit_action
    }

    #[must_use]
    pub fn audit_target_ref(&self) -> Option<&str> {
        self.audit_target_ref.as_deref()
    }

    #[must_use]
    pub fn canonical_request(&self) -> &[u8] {
        &self.canonical_request
    }

    #[must_use]
    pub const fn idempotency_expires_at(&self) -> DateTime<Utc> {
        self.idempotency_expires_at
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AdmissionInputError {
    #[error("{field} is required")]
    Empty { field: &'static str },
    #[error("{field} exceeds {maximum} bytes")]
    TooLong { field: &'static str, maximum: usize },
    #[error("canonical request exceeds the 1 MiB admission limit")]
    CanonicalRequestTooLarge,
    #[error("idempotency expiry must be in the future")]
    ExpiredIdempotencyRecord,
}

fn validated_text(
    field: &'static str,
    value: String,
    maximum: usize,
) -> Result<String, AdmissionInputError> {
    if value.trim().is_empty() {
        return Err(AdmissionInputError::Empty { field });
    }
    if value.len() > maximum {
        return Err(AdmissionInputError::TooLong { field, maximum });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::{AdmissionInputError, AdmitOperationRequest, OperationState};
    use crate::auth::domain::PrincipalContext;

    #[test]
    fn operation_states_have_stable_persistence_values() {
        for (value, state) in [
            ("recoverable", OperationState::Recoverable),
            ("running", OperationState::Running),
            ("uncertain", OperationState::Uncertain),
            ("succeeded", OperationState::Succeeded),
            ("failed", OperationState::Failed),
            ("cancelled", OperationState::Cancelled),
        ] {
            assert_eq!(OperationState::try_from(value).unwrap(), state);
            assert_eq!(state.as_str(), value);
        }
    }

    #[test]
    fn admission_rejects_an_expired_idempotency_record() {
        let error = AdmitOperationRequest::new(
            PrincipalContext::new(uuid::Uuid::now_v7()),
            "operations.create",
            "idempotency-key",
            "test.operation",
            "operation.admit",
            None,
            b"{}".to_vec(),
            Utc::now() - Duration::seconds(1),
        )
        .unwrap_err();

        assert_eq!(error, AdmissionInputError::ExpiredIdempotencyRecord);
    }
}
