use chrono::SecondsFormat;
use loco_rs::prelude::*;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::operations::{
    actions::get::{self, GetOperationError},
    domain::{OperationSnapshot, OperationState},
};

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OperationResponse {
    pub id: String,
    pub kind: String,
    pub state: OperationStateResponse,
    pub recoverable: bool,
    pub uncertain: bool,
    pub last_event_sequence: i32,
    pub last_error_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationStateResponse {
    Recoverable,
    Running,
    Uncertain,
    Succeeded,
    Failed,
    Cancelled,
}

impl From<OperationState> for OperationStateResponse {
    fn from(state: OperationState) -> Self {
        match state {
            OperationState::Recoverable => Self::Recoverable,
            OperationState::Running => Self::Running,
            OperationState::Uncertain => Self::Uncertain,
            OperationState::Succeeded => Self::Succeeded,
            OperationState::Failed => Self::Failed,
            OperationState::Cancelled => Self::Cancelled,
        }
    }
}

impl From<OperationSnapshot> for OperationResponse {
    fn from(snapshot: OperationSnapshot) -> Self {
        let operation = snapshot.operation;
        Self {
            id: operation.id.to_string(),
            kind: operation.kind,
            state: operation.state.into(),
            recoverable: operation.state.is_recoverable(),
            uncertain: operation.state.is_uncertain(),
            last_event_sequence: snapshot.last_event_sequence,
            last_error_code: snapshot.last_error_code,
            created_at: operation
                .created_at
                .to_rfc3339_opts(SecondsFormat::Millis, true),
            updated_at: operation
                .updated_at
                .to_rfc3339_opts(SecondsFormat::Millis, true),
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/operations/{operation_id}",
    params(("operation_id" = String, Path, description = "Durable operation UUID")),
    responses(
        (status = 200, description = "Current durable operation state", body = OperationResponse),
        (status = 400, description = "Invalid operation identifier"),
        (status = 404, description = "Operation not found"),
        (status = 500, description = "Operation state unavailable")
    )
)]
pub(crate) async fn show(
    State(context): State<AppContext>,
    Path(operation_id): Path<String>,
) -> Result<Response> {
    let operation_id = Uuid::parse_str(&operation_id)
        .map_err(|_| Error::BadRequest("operation id is invalid".to_owned()))?;
    let snapshot = get::execute(&context.db, operation_id)
        .await
        .map_err(map_get_error)?;

    format::json(OperationResponse::from(snapshot))
}

fn map_get_error(error: GetOperationError) -> Error {
    match error {
        GetOperationError::NotFound => Error::NotFound,
        GetOperationError::MissingEvent
        | GetOperationError::CorruptState(_)
        | GetOperationError::Persistence(_) => Error::InternalServerError,
    }
}

pub fn routes() -> Routes {
    Routes::new().get("/operations/{operation_id}", show)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    use super::{OperationResponse, OperationStateResponse};
    use crate::operations::domain::{DurableOperation, OperationSnapshot, OperationState};

    #[test]
    fn polling_dto_is_distinct_from_the_persistence_model() {
        let operation_id = Uuid::now_v7();
        let timestamp = Utc.with_ymd_and_hms(2026, 8, 23, 12, 0, 0).unwrap();
        let response = OperationResponse::from(OperationSnapshot {
            operation: DurableOperation {
                id: operation_id,
                kind: "service.restart".to_owned(),
                state: OperationState::Uncertain,
                claimed_by: Some("controller:test".to_owned()),
                claimed_at: Some(timestamp),
                created_at: timestamp,
                updated_at: timestamp,
            },
            last_event_sequence: 3,
            last_error_code: Some("engine_timeout".to_owned()),
        });

        assert_eq!(response.id, operation_id.to_string());
        assert_eq!(response.state, OperationStateResponse::Uncertain);
        assert!(response.uncertain);
        assert!(!response.recoverable);
        assert_eq!(response.last_error_code.as_deref(), Some("engine_timeout"));
    }
}
