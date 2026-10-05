use axum::{extract::RawQuery, http::StatusCode};
use chrono::{DateTime, SecondsFormat, Utc};
use loco_rs::{environment::Environment, prelude::*};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::admission::{Admission, AdmittingRequestId, IdempotencyKey, admission_error, admit};
use crate::{
    auth::domain::PrincipalContext,
    operations::{
        actions::{
            cancel::{self, CancelOperationError},
            get::{self, GetOperationError},
            list_events::{self, ListEventsError, MAX_EVENT_PAGE},
        },
        domain::{OperationEventView, OperationKind, OperationSnapshot, OperationState},
        executors::noop::NoopRequest,
    },
};

fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

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
    /// Failure message of a failed operation; never daemon or transport internals.
    pub error_message: Option<String>,
    /// The latest progress or result document for the kind.
    #[schema(value_type = Option<Object>)]
    pub progress: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
    /// When a worker claimed it.
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    /// The admitting request's id, shown next to failures.
    pub request_id: Option<String>,
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
            error_message: snapshot.error_message,
            progress: snapshot.progress,
            created_at: timestamp(operation.created_at),
            updated_at: timestamp(operation.updated_at),
            started_at: operation.claimed_at.map(timestamp),
            finished_at: operation.finished_at.map(timestamp),
            request_id: operation.request_id,
        }
    }
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OperationEventResponse {
    pub sequence: i32,
    pub state: OperationStateResponse,
    pub error_code: Option<String>,
    pub message: Option<String>,
    #[schema(value_type = Option<Object>)]
    pub detail: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
pub struct OperationEventsResponse {
    #[schema(max_items = 100)]
    pub items: Vec<OperationEventResponse>,
}

impl From<OperationEventView> for OperationEventResponse {
    fn from(event: OperationEventView) -> Self {
        Self {
            sequence: event.sequence,
            state: event.state.into(),
            error_code: event.error_code,
            message: event.message,
            detail: event.detail,
            created_at: timestamp(event.created_at),
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/operations/{operation_id}",
    operation_id = "operation",
    params(("operation_id" = String, Path, description = "Durable operation UUID")),
    responses(
        (status = 200, description = "Current durable operation state", body = OperationResponse),
        (status = 400, description = "Invalid operation identifier"),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 404, description = "Operation not found or not visible to the authenticated principal"),
        (status = 500, description = "Operation state unavailable"),
        (status = 503, description = "Controller draining new work or session persistence unavailable; the body is either HealthResponse or a generic error")
    )
)]
pub(crate) async fn show(
    State(context): State<AppContext>,
    principal: PrincipalContext,
    Path(operation_id): Path<String>,
) -> Result<Response> {
    let operation_id = Uuid::parse_str(&operation_id)
        .map_err(|_| Error::BadRequest("operation id is invalid".to_owned()))?;
    let snapshot = get::execute(&context.db, &principal, operation_id)
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

fn parse_operation_id(raw: &str) -> Option<Uuid> {
    Uuid::parse_str(raw).ok()
}

/// `after` (default 0) and `limit` (default 100, at most 100); nothing else.
fn parse_event_query(raw: Option<&str>) -> Option<(i32, u64)> {
    let mut after = None;
    let mut limit = None;
    for (key, value) in url::form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
        let parsed = value.parse::<u64>().ok()?;
        let slot = match key.as_ref() {
            "after" => &mut after,
            "limit" => &mut limit,
            _ => return None,
        };
        if slot.replace(parsed).is_some() {
            return None;
        }
    }
    let after = i32::try_from(after.unwrap_or(0)).ok()?;
    let limit = limit.unwrap_or(MAX_EVENT_PAGE);
    (1..=MAX_EVENT_PAGE)
        .contains(&limit)
        .then_some((after, limit))
}

#[utoipa::path(
    get,
    path = "/api/v1/operations/{operation_id}/events",
    operation_id = "operation_events",
    params(
        ("operation_id" = String, Path, description = "Durable operation UUID"),
        ("after" = Option<u32>, Query, description = "Return events after this sequence (default 0)"),
        ("limit" = Option<u32>, Query, description = "At most this many events, 1–100 (default 100)")
    ),
    responses(
        (status = 200, description = "Events in sequence order", body = OperationEventsResponse),
        (status = 400, description = "Invalid operation identifier or query", body = super::admission::AdmissionErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 404, description = "Operation not found or not visible to the authenticated principal"),
        (status = 503, description = "Persistence unavailable or controller draining")
    )
)]
pub(crate) async fn events(
    State(context): State<AppContext>,
    principal: PrincipalContext,
    Path(operation_id): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    let (Some(operation_id), Some((after, limit))) = (
        parse_operation_id(&operation_id),
        parse_event_query(query.as_deref()),
    ) else {
        return admission_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    match list_events::execute(&context.db, &principal, operation_id, after, limit).await {
        Ok(events) => (
            [(axum::http::header::CACHE_CONTROL, "no-store")],
            axum::Json(OperationEventsResponse {
                items: events.into_iter().map(Into::into).collect(),
            }),
        )
            .into_response(),
        Err(ListEventsError::NotFound) => admission_error(StatusCode::NOT_FOUND, "not_found"),
        Err(ListEventsError::InvalidLimit) => {
            admission_error(StatusCode::BAD_REQUEST, "invalid_request")
        }
        Err(ListEventsError::CorruptState(_) | ListEventsError::Persistence(_)) => {
            admission_error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable")
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/operations/{operation_id}/cancel",
    operation_id = "cancel_operation",
    params(("operation_id" = String, Path, description = "Durable operation UUID")),
    responses(
        (status = 202, description = "Cancelled before it started; the body is the operation", body = OperationResponse),
        (status = 400, description = "Invalid operation identifier", body = super::admission::AdmissionErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 404, description = "Operation not found or not visible to the authenticated principal"),
        (status = 409, description = "`not_cancellable`: the operation already started or finished", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "Persistence unavailable or controller draining")
    )
)]
pub(crate) async fn cancel(
    State(context): State<AppContext>,
    principal: PrincipalContext,
    Path(operation_id): Path<String>,
) -> Response {
    let Some(operation_id) = parse_operation_id(&operation_id) else {
        return admission_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    match cancel::execute(&context.db, &principal, operation_id).await {
        Ok(_) => match get::execute(&context.db, &principal, operation_id).await {
            Ok(snapshot) => (
                StatusCode::ACCEPTED,
                [(axum::http::header::CACHE_CONTROL, "no-store")],
                axum::Json(OperationResponse::from(snapshot)),
            )
                .into_response(),
            Err(_) => admission_error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable"),
        },
        Err(CancelOperationError::NotFound) => admission_error(StatusCode::NOT_FOUND, "not_found"),
        Err(CancelOperationError::NotCancellable) => {
            admission_error(StatusCode::CONFLICT, "not_cancellable")
        }
        Err(CancelOperationError::CorruptState(_) | CancelOperationError::Persistence(_)) => {
            admission_error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable")
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/operations/noop",
    operation_id = "admit_noop",
    params(("Idempotency-Key" = String, Header, description = "16–64 printable ASCII characters, one per user gesture")),
    request_body = Object,
    responses(
        (status = 202, description = "Admitted; poll the operation", body = super::admission::AdmittedResponse),
        (status = 400, description = "`idempotency_key_required` or `invalid_request`", body = super::admission::AdmissionErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 404, description = "Not available outside development and test"),
        (status = 409, description = "`idempotency_conflict`: the key was used for a different request", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "Persistence unavailable or controller draining")
    )
)]
pub(crate) async fn admit_noop(
    State(context): State<AppContext>,
    principal: PrincipalContext,
    key: IdempotencyKey,
    AdmittingRequestId(request_id): AdmittingRequestId,
    body: axum::body::Bytes,
) -> Response {
    if !matches!(
        context.environment,
        Environment::Development | Environment::Test
    ) {
        return admission_error(StatusCode::NOT_FOUND, "not_found");
    }
    let Ok(request) = serde_json::from_slice::<NoopRequest>(&body) else {
        return admission_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    if request.validate().is_err() {
        return admission_error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    let payload = serde_json::to_value(&request).unwrap_or_default();
    admit(
        &context,
        Admission {
            principal,
            key,
            request_id,
            scope: "/operations/noop",
            kind: OperationKind::Noop,
            payload,
        },
    )
    .await
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/operations/noop", post(admit_noop))
        .get("/operations/{operation_id}", show)
        .get("/operations/{operation_id}/events", events)
        .add("/operations/{operation_id}/cancel", post(cancel))
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    use super::{OperationResponse, OperationStateResponse, parse_event_query};

    #[test]
    fn event_queries_default_and_bound() {
        assert_eq!(parse_event_query(None), Some((0, 100)));
        assert_eq!(parse_event_query(Some("after=3&limit=10")), Some((3, 10)));
        for bad in [
            "limit=0",
            "limit=101",
            "after=-1",
            "after=x",
            "other=1",
            "after=1&after=2",
        ] {
            assert_eq!(parse_event_query(Some(bad)), None, "{bad}");
        }
    }
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
                payload: serde_json::json!({"secret": "payload-canary"}),
                finished_at: None,
                request_id: Some("01J9ZM2R".to_owned()),
            },
            last_event_sequence: 3,
            last_error_code: Some("engine_timeout".to_owned()),
            progress: Some(serde_json::json!({"done": 1, "total": 3})),
            error_message: None,
        });
        assert!(
            !serde_json::to_string(&response)
                .unwrap()
                .contains("payload-canary"),
            "the stored payload never crosses the polling DTO"
        );
        assert_eq!(
            response.started_at.as_deref(),
            Some("2026-08-23T12:00:00.000Z")
        );
        assert_eq!(response.request_id.as_deref(), Some("01J9ZM2R"));

        assert_eq!(response.id, operation_id.to_string());
        assert_eq!(response.state, OperationStateResponse::Uncertain);
        assert!(response.uncertain);
        assert!(!response.recoverable);
        assert_eq!(response.last_error_code.as_deref(), Some("engine_timeout"));
    }
}
