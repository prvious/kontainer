use std::time::Duration;

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

impl OperationState {
    /// `succeeded`, `failed`, and `cancelled` never change again.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

/// Every mutation the controller runs. Specs 07 to 12 register executors for their kinds;
/// `noop` exists for the operation pipeline itself (development and test routes only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationKind {
    ContainerStart,
    ContainerStop,
    ContainerRestart,
    ContainerPause,
    ContainerUnpause,
    ContainerRemove,
    ContainersBulk,
    ImageRemove,
    ImagePull,
    VolumeCreate,
    VolumeRemove,
    NetworkCreate,
    NetworkRemove,
    ServiceScale,
    ServiceRestart,
    ServiceRollback,
    StackRemove,
    Noop,
}

impl OperationKind {
    pub const ALL: [Self; 18] = [
        Self::ContainerStart,
        Self::ContainerStop,
        Self::ContainerRestart,
        Self::ContainerPause,
        Self::ContainerUnpause,
        Self::ContainerRemove,
        Self::ContainersBulk,
        Self::ImageRemove,
        Self::ImagePull,
        Self::VolumeCreate,
        Self::VolumeRemove,
        Self::NetworkCreate,
        Self::NetworkRemove,
        Self::ServiceScale,
        Self::ServiceRestart,
        Self::ServiceRollback,
        Self::StackRemove,
        Self::Noop,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ContainerStart => "container.start",
            Self::ContainerStop => "container.stop",
            Self::ContainerRestart => "container.restart",
            Self::ContainerPause => "container.pause",
            Self::ContainerUnpause => "container.unpause",
            Self::ContainerRemove => "container.remove",
            Self::ContainersBulk => "containers.bulk",
            Self::ImageRemove => "image.remove",
            Self::ImagePull => "image.pull",
            Self::VolumeCreate => "volume.create",
            Self::VolumeRemove => "volume.remove",
            Self::NetworkCreate => "network.create",
            Self::NetworkRemove => "network.remove",
            Self::ServiceScale => "service.scale",
            Self::ServiceRestart => "service.restart",
            Self::ServiceRollback => "service.rollback",
            Self::StackRemove => "stack.remove",
            Self::Noop => "noop",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }

    /// Wall-clock budget for one execution: pulls get ten minutes, everything else one.
    #[must_use]
    pub const fn timeout(self) -> Duration {
        match self {
            Self::ImagePull => Duration::from_secs(10 * 60),
            // The 60 s convergence wait plus the write and a last poll.
            Self::ServiceScale | Self::ServiceRestart | Self::ServiceRollback => {
                Duration::from_secs(90)
            }
            // Up to 60 s for the services' tasks to go, then up to 60 s for the networks.
            Self::StackRemove => Duration::from_secs(150),
            _ => Duration::from_secs(60),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurableOperation {
    pub id: Uuid,
    pub kind: String,
    pub state: OperationState,
    pub claimed_by: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub payload: serde_json::Value,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub request_id: Option<String>,
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
            payload: model.payload,
            started_at: model.started_at,
            finished_at: model.finished_at,
            request_id: model.request_id,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationSnapshot {
    pub operation: DurableOperation,
    pub last_event_sequence: i32,
    pub last_error_code: Option<String>,
    /// The latest progress or result detail, if any event carried one.
    pub progress: Option<serde_json::Value>,
    /// The failure message of a failed operation.
    pub error_message: Option<String>,
}

/// One entry of an operation's history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationEventView {
    pub sequence: i32,
    pub state: OperationState,
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub detail: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
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
    payload: serde_json::Value,
    request_id: Option<String>,
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
            payload: serde_json::Value::Object(serde_json::Map::new()),
            request_id: None,
        })
    }

    /// The validated request the executor will read. Defaults to `{}`.
    #[must_use]
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }

    /// The admitting HTTP request id; bounded so a hostile header cannot bloat the row.
    #[must_use]
    pub fn with_request_id(mut self, request_id: Option<String>) -> Self {
        self.request_id = request_id.filter(|id| {
            !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_graphic())
        });
        self
    }

    #[must_use]
    pub fn payload(&self) -> &serde_json::Value {
        &self.payload
    }

    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
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
    fn kinds_round_trip_and_pulls_get_ten_minutes() {
        use super::OperationKind;
        for kind in OperationKind::ALL {
            assert_eq!(OperationKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(OperationKind::parse("container.explode"), None);
        assert_eq!(OperationKind::ImagePull.timeout().as_secs(), 600);
        assert_eq!(OperationKind::ContainerRestart.timeout().as_secs(), 60);
        assert!(OperationState::Failed.is_terminal());
        assert!(!OperationState::Uncertain.is_terminal());
    }

    #[test]
    fn request_ids_are_bounded_printable_text() {
        let request = || {
            AdmitOperationRequest::new(
                PrincipalContext::new(uuid::Uuid::now_v7()),
                "operations.create",
                "idempotency-key",
                "noop",
                "operation.admit",
                None,
                b"{}".to_vec(),
                Utc::now() + Duration::hours(1),
            )
            .unwrap()
        };
        assert_eq!(
            request()
                .with_request_id(Some("01J9ZM2R".to_owned()))
                .request_id(),
            Some("01J9ZM2R")
        );
        for bad in ["", "has space", "\u{7f}", &"x".repeat(129)] {
            assert_eq!(
                request().with_request_id(Some(bad.to_owned())).request_id(),
                None
            );
        }
        assert_eq!(request().payload(), &serde_json::json!({}));
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
