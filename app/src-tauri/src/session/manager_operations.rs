//! Durable operations and their admissions as credential-free IPC views.
//!
//! Mirrors `app/src/types/operations.ts`; the shared fixture
//! `app/tests/fixtures/operations-ipc.json` keeps both sides in sync.

use kontainer_native_client::{AdmittedOperation, OperationSnapshot, OperationState};
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
}
