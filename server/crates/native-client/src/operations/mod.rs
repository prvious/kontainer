//! Owner-scoped durable operation polling.
//!
//! The controller scopes polling to the authenticated principal: another
//! principal's operation and a nonexistent operation are indistinguishable
//! (`404`), which this client passes through unchanged.

mod get;

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
    /// Highest persisted transition sequence.
    pub last_event_sequence: i32,
    /// Stable error code of the last failure, when one exists.
    pub last_error_code: Option<String>,
    /// Admission time reported by the controller.
    pub created_at: DateTime<Utc>,
    /// Last transition time reported by the controller.
    pub updated_at: DateTime<Utc>,
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
    created_at: String,
    updated_at: String,
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
            last_event_sequence: self.last_event_sequence,
            last_error_code: self.last_error_code,
            created_at: parse_timestamp(&self.created_at)?,
            updated_at: parse_timestamp(&self.updated_at)?,
        })
    }
}
