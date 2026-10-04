//! Mutation admission at the HTTP edge: every mutation route takes an `Idempotency-Key`,
//! validates its body, and answers 202 with the operation id to poll.

use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
};
use chrono::{Duration, Utc};
use loco_rs::app::AppContext;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    auth::domain::PrincipalContext,
    operations::{
        actions::admit::{self, AdmitOperationError},
        domain::{AdmitOperationRequest, DurableOperation, OperationKind},
    },
};

pub const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";
/// Replays within this window return the original operation; it matches operation retention.
const IDEMPOTENCY_WINDOW: Duration = Duration::days(7);

/// `Idempotency-Key`: 16–64 printable ASCII characters, one per user gesture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyKey(pub String);

impl IdempotencyKey {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        ((16..=64).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_graphic()))
            .then(|| Self(value.to_owned()))
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AdmissionErrorResponse {
    pub error: String,
}

/// A 400 naming the field that failed: `{ "error": "invalid_request", "fields": { field: problem } }`.
#[derive(Debug, Serialize, ToSchema)]
pub struct FieldErrorResponse {
    pub error: String,
    #[schema(value_type = Object)]
    pub fields: serde_json::Value,
}

pub(crate) fn field_error(field: &str, problem: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        [(header::CACHE_CONTROL, "no-store")],
        Json(FieldErrorResponse {
            error: "invalid_request".to_owned(),
            fields: serde_json::json!({ field: problem }),
        }),
    )
        .into_response()
}

pub(crate) fn admission_error(status: StatusCode, code: &str) -> Response {
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Json(AdmissionErrorResponse {
            error: code.to_owned(),
        }),
    )
        .into_response()
}

impl<S: Send + Sync> FromRequestParts<S> for IdempotencyKey {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .headers
            .get(IDEMPOTENCY_KEY_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(Self::parse)
            .ok_or_else(|| admission_error(StatusCode::BAD_REQUEST, "idempotency_key_required"))
    }
}

/// The admitting request's id, when Loco's request-id middleware set one.
pub struct AdmittingRequestId(pub Option<String>);

impl<S: Send + Sync> FromRequestParts<S> for AdmittingRequestId {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<loco_rs::controller::middleware::request_id::LocoRequestId>()
                .map(|id| id.get().to_owned()),
        ))
    }
}

#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AdmittedResponse {
    pub operation_id: String,
    /// True when this key already admitted the same request; no second operation exists.
    pub replayed: bool,
}

/// One admitted mutation: the scope is the route path with ids, the payload the validated body.
pub struct Admission<'a> {
    pub principal: PrincipalContext,
    pub key: IdempotencyKey,
    pub request_id: Option<String>,
    pub scope: &'a str,
    pub kind: OperationKind,
    pub payload: serde_json::Value,
}

/// Admit and answer 202 `{ operationId, replayed }`, or 409 `idempotency_conflict` when the key
/// was used for a different body.
pub(crate) async fn admit(context: &AppContext, admission: Admission<'_>) -> Response {
    let canonical = serde_json::to_vec(&admission.payload).unwrap_or_default();
    let request = match AdmitOperationRequest::new(
        admission.principal,
        admission.scope,
        admission.key.0,
        admission.kind.as_str(),
        admission.kind.as_str(),
        Some(admission.scope.to_owned()),
        canonical,
        Utc::now() + IDEMPOTENCY_WINDOW,
    ) {
        Ok(request) => request
            .with_payload(admission.payload)
            .with_request_id(admission.request_id),
        Err(_) => return admission_error(StatusCode::BAD_REQUEST, "invalid_request"),
    };
    match admit::execute(&context.db, request).await {
        Ok(outcome) => admitted(&outcome.operation, outcome.replayed),
        Err(AdmitOperationError::IdempotencyConflict { .. }) => {
            admission_error(StatusCode::CONFLICT, "idempotency_conflict")
        }
        Err(error) => {
            tracing::warn!(%error, "operation admission failed");
            admission_error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable")
        }
    }
}

/// Answer a retried mutation before the route's live preconditions run, since the original
/// operation may have changed what they check. `request` holds the fields the client sent, as the
/// payload names them: the original 202 when this key admitted the same kind with those values, 409
/// `idempotency_conflict` when it admitted something else, and `None` for a new key.
pub(crate) async fn replay_existing(
    context: &AppContext,
    principal: &PrincipalContext,
    scope: &str,
    key: &IdempotencyKey,
    kind: OperationKind,
    request: &serde_json::Value,
) -> Option<Response> {
    let operation = match admit::find_admitted(&context.db, principal, scope, &key.0).await {
        Ok(operation) => operation?,
        Err(error) => {
            tracing::warn!(%error, "idempotency lookup failed");
            return Some(admission_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "persistence_unavailable",
            ));
        }
    };
    let same_request = operation.kind == kind.as_str()
        && request.as_object().is_some_and(|fields| {
            fields
                .iter()
                .all(|(field, value)| operation.payload.get(field) == Some(value))
        });
    Some(if same_request {
        admitted(&operation, true)
    } else {
        admission_error(StatusCode::CONFLICT, "idempotency_conflict")
    })
}

fn admitted(operation: &DurableOperation, replayed: bool) -> Response {
    (
        StatusCode::ACCEPTED,
        [(header::CACHE_CONTROL, "no-store")],
        Json(AdmittedResponse {
            operation_id: operation.id.to_string(),
            replayed,
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::IdempotencyKey;

    #[test]
    fn keys_are_sixteen_to_sixty_four_printable_characters() {
        assert!(IdempotencyKey::parse("018f2e34-0000-7000-8000-0000000000aa").is_some());
        assert!(IdempotencyKey::parse(&"k".repeat(16)).is_some());
        assert!(IdempotencyKey::parse(&"k".repeat(64)).is_some());
        for bad in [
            "",
            "short",
            &"k".repeat(15),
            &"k".repeat(65),
            "has spaces in it ok?",
            "tab\tin-the-key-here",
        ] {
            assert!(IdempotencyKey::parse(bad).is_none(), "{bad:?}");
        }
    }
}
