//! Durable operations as credential-free IPC views, and the admission path every mutation
//! command uses.
//!
//! Mirrors `app/src/types/operations.ts`; the shared fixture
//! `app/tests/fixtures/operations-ipc.json` keeps both sides in sync.

use kontainer_native_client::{
    AdmittedOperation, IdempotencyKey, OperationEvent, OperationSnapshot, OperationState,
};
use serde::Serialize;
use uuid::Uuid;

use super::{inventory::InventoryError, SessionManager};

/// The controller's canonical RFC 3339 millisecond form.
macro_rules! timestamp {
    ($value:expr) => {
        $value.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
    };
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OperationView {
    id: String,
    kind: String,
    state: OperationState,
    last_event_sequence: i32,
    error_code: Option<String>,
    error_message: Option<String>,
    progress: Option<serde_json::Value>,
    created_at: String,
    updated_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    request_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OperationEventView {
    sequence: i32,
    state: OperationState,
    error_code: Option<String>,
    message: Option<String>,
    detail: Option<serde_json::Value>,
    created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AdmittedView {
    operation_id: String,
    replayed: bool,
}

impl From<OperationSnapshot> for OperationView {
    fn from(snapshot: OperationSnapshot) -> Self {
        Self {
            id: snapshot.id.to_string(),
            kind: snapshot.kind,
            state: snapshot.state,
            last_event_sequence: snapshot.last_event_sequence,
            error_code: snapshot.last_error_code,
            error_message: snapshot.error_message,
            progress: snapshot.progress,
            created_at: timestamp!(snapshot.created_at),
            updated_at: timestamp!(snapshot.updated_at),
            started_at: snapshot.started_at.map(|at| timestamp!(at)),
            finished_at: snapshot.finished_at.map(|at| timestamp!(at)),
            request_id: snapshot.request_id,
        }
    }
}

impl From<OperationEvent> for OperationEventView {
    fn from(event: OperationEvent) -> Self {
        Self {
            sequence: event.sequence,
            state: event.state,
            error_code: event.error_code,
            message: event.message,
            detail: event.detail,
            created_at: timestamp!(event.created_at),
        }
    }
}

impl From<AdmittedOperation> for AdmittedView {
    fn from(admitted: AdmittedOperation) -> Self {
        Self {
            operation_id: admitted.operation_id.to_string(),
            replayed: admitted.replayed,
        }
    }
}

/// Operation reads answer "not found", never "node not found".
fn operation_error(error: InventoryError) -> InventoryError {
    match error {
        InventoryError::NodeNotFound => InventoryError::NotFound,
        other => other,
    }
}

fn parse_operation_id(value: &str) -> Result<Uuid, InventoryError> {
    Uuid::parse_str(value).map_err(|_| InventoryError::InvalidRequest)
}

impl SessionManager {
    pub(crate) async fn operation(
        &self,
        profile_id: Uuid,
        operation_id: &str,
    ) -> Result<OperationView, InventoryError> {
        let id = parse_operation_id(operation_id)?;
        let snapshot = self
            .authenticated(profile_id, &id, |controller, access, cancellation, id| {
                controller.get_operation(access, *id, cancellation)
            })
            .await
            .map_err(operation_error)?;
        Ok(snapshot.into())
    }

    pub(crate) async fn operation_events(
        &self,
        profile_id: Uuid,
        operation_id: &str,
        after: u32,
    ) -> Result<Vec<OperationEventView>, InventoryError> {
        let id = parse_operation_id(operation_id)?;
        let events = self
            .authenticated(
                profile_id,
                &(id, after),
                |controller, access, cancellation, (id, after)| {
                    controller.list_operation_events(access, *id, *after, cancellation)
                },
            )
            .await
            .map_err(operation_error)?;
        Ok(events.into_iter().map(OperationEventView::from).collect())
    }

    pub(crate) async fn cancel_operation(
        &self,
        profile_id: Uuid,
        operation_id: &str,
    ) -> Result<OperationView, InventoryError> {
        let id = parse_operation_id(operation_id)?;
        let snapshot = self
            .authenticated(profile_id, &id, |controller, access, cancellation, id| {
                controller.cancel_operation(access, *id, cancellation)
            })
            .await
            .map_err(operation_error)?;
        Ok(snapshot.into())
    }

    /// Admit one mutation. The key comes from the user gesture in React, so a retried command
    /// replays instead of mutating twice; a rotated credential retries with the same key.
    // Specs 07 to 12 call this from their mutation commands.
    #[allow(dead_code)]
    pub(crate) async fn admit(
        &self,
        profile_id: Uuid,
        path: &str,
        idempotency_key: &str,
        body: serde_json::Value,
    ) -> Result<AdmittedView, InventoryError> {
        let key = IdempotencyKey::parse(idempotency_key)?;
        let admitted = self
            .authenticated(
                profile_id,
                &(path, key, body),
                |controller, access, cancellation, (path, key, body)| {
                    controller.admit(access, path, key, body, cancellation)
                },
            )
            .await?;
        Ok(admitted.into())
    }
}
