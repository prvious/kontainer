//! Owner-scoped durable operation polling.
//!
//! The controller scopes polling to the authenticated principal: another
//! principal's operation and a nonexistent operation are indistinguishable
//! (`404`), which this client passes through unchanged.

mod admit;
mod cancel;
mod get;

pub use admit::{AdmittedOperation, IdempotencyKey};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client::parse_timestamp;
use crate::error::NativeClientError;

/// Persisted durable-operation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    /// Waiting to be claimed and safe to retry.
    Recoverable,
    /// Claimed and executing.
    Running,
    /// The outcome cannot be proven without inspection.
    Uncertain,
    /// Completed successfully.
    Succeeded,
    /// Completed with a failure.
    Failed,
    /// Cancelled before completion.
    Cancelled,
}

/// One bounded snapshot of a durable operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationSnapshot {
    /// Operation identifier, echoed by the controller.
    pub id: Uuid,
    /// Operation kind, for example `service.restart`.
    pub kind: String,
    /// Current typed state.
    pub state: OperationState,
    /// Whether the operation is in the recoverable state.
    pub recoverable: bool,
    /// Whether the outcome is uncertain and requires inspection.
    pub uncertain: bool,
    /// Stable error code of the last failure, when one exists.
    pub last_error_code: Option<String>,
    /// Admission time reported by the controller.
    pub created_at: DateTime<Utc>,
    /// Last transition time reported by the controller.
    pub updated_at: DateTime<Utc>,
    /// Failure message of a failed operation.
    pub error_message: Option<String>,
    /// The latest progress or result document for the kind.
    pub progress: Option<serde_json::Value>,
    /// When a worker claimed it.
    pub started_at: Option<DateTime<Utc>>,
    /// When it reached a terminal state.
    pub finished_at: Option<DateTime<Utc>>,
    /// The admitting request's id.
    pub request_id: Option<String>,
}

impl OperationState {
    /// `succeeded`, `failed`, and `cancelled` never change again.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OperationBody {
    id: String,
    kind: String,
    state: OperationState,
    recoverable: bool,
    uncertain: bool,
    last_event_sequence: i32,
    last_error_code: Option<String>,
    error_message: Option<String>,
    progress: Option<serde_json::Value>,
    created_at: String,
    updated_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    request_id: Option<String>,
}

impl OperationBody {
    fn into_snapshot(self, requested: Uuid) -> Result<OperationSnapshot, NativeClientError> {
        let id = crate::client::parse_uuid(&self.id)?;
        if id != requested {
            return Err(NativeClientError::InvalidResponse);
        }
        // The controller derives both flags from the state; disagreement means
        // the response is not the documented snapshot.
        if self.recoverable != (self.state == OperationState::Recoverable)
            || self.uncertain != (self.state == OperationState::Uncertain)
        {
            return Err(NativeClientError::InvalidResponse);
        }
        Ok(OperationSnapshot {
            id,
            kind: self.kind,
            state: self.state,
            recoverable: self.recoverable,
            uncertain: self.uncertain,
            last_error_code: self.last_error_code,
            created_at: parse_timestamp(&self.created_at)?,
            updated_at: parse_timestamp(&self.updated_at)?,
            error_message: self.error_message,
            progress: self.progress.filter(serde_json::Value::is_object),
            started_at: self
                .started_at
                .as_deref()
                .map(parse_timestamp)
                .transpose()?,
            finished_at: self
                .finished_at
                .as_deref()
                .map(parse_timestamp)
                .transpose()?,
            request_id: self.request_id,
        })
    }
}
