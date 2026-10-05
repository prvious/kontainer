#![forbid(unsafe_code)]

//! Swarm services from the manager: the list with running counts, one service with its tasks,
//! and scale / restart / rollback as durable operations. When the manager can't answer, reads
//! and writes are 503 `quorum_unavailable` and no operation is admitted.

use axum::{
    Json,
    extract::{Path, RawQuery, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, SecondsFormat, Utc};
use kontainer_engine::{ServiceMode, TaskSummary, image_without_digest, is_valid_swarm_id};
use loco_rs::{app::AppContext, controller::Routes, prelude::post};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use utoipa::ToSchema;

use super::{
    admission::{
        Admission, AdmittingRequestId, IdempotencyKey, admission_error, admit, field_error,
        replay_existing,
    },
    nodes::json_no_store,
};
use crate::{
    auth::domain::PrincipalContext,
    operations::domain::OperationKind,
    services::{
        actions::{get_service, list_services},
        domain::{ServiceAction, ServiceActionPayload, ServiceRow},
    },
    swarm::engine::{ManagerError, SharedManagerEngine},
};

/// Seconds the desktop waits before asking again when the swarm has no quorum.
const QUORUM_RETRY_SECONDS: u32 = 10;
const MAX_REPLICAS: u64 = 1_000;
/// Tasks returned with one service, newest first.
const MAX_TASKS: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceResponse {
    pub id: String,
    pub name: String,
    /// Shown without the digest.
    pub image: String,
    /// The full reference the tasks run, digest included.
    pub image_reference: String,
    /// `replicated` or `global`.
    pub mode: String,
    /// Replicated: the replicas asked for. Global: one per eligible node.
    pub desired_replicas: u64,
    pub running_replicas: u64,
    /// Published ports, `published:target/proto`.
    pub ports: Vec<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// Swarm's object version; writes send it back.
    pub version: u64,
    pub stack: Option<String>,
    /// Swarm's update status, e.g. `updating`, `completed`, `rollback_completed`.
    pub update_state: Option<String>,
    pub update_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceListResponse {
    pub items: Vec<ServiceResponse>,
    pub snapshot_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponse {
    pub id: String,
    pub slot: Option<u64>,
    /// Swarm node id (not the Kontainer node id).
    pub swarm_node_id: Option<String>,
    pub state: String,
    pub desired_state: String,
    pub error: Option<String>,
    pub message: Option<String>,
    pub updated_at: Option<String>,
    pub container_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDetailResponse {
    pub service: ServiceResponse,
    /// Newest first, at most 50.
    pub tasks: Vec<TaskResponse>,
    /// The newest task error, for the replica tooltip and the scaling status line.
    pub latest_error: Option<String>,
}

/// 503 while the manager can't answer; the desktop retries after `retryAfterSeconds`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuorumUnavailableResponse {
    pub error: String,
    pub retry_after_seconds: u32,
}

fn timestamp(seconds: Option<i64>) -> Option<String> {
    seconds
        .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0))
        .map(|at| at.to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn service_response(row: ServiceRow) -> ServiceResponse {
    let service = row.service;
    ServiceResponse {
        image: image_without_digest(&service.image).to_owned(),
        image_reference: service.image,
        mode: match service.mode {
            ServiceMode::Replicated { .. } => "replicated",
            ServiceMode::Global => "global",
        }
        .to_owned(),
        desired_replicas: row.desired,
        running_replicas: row.running,
        created_at: timestamp(service.created_at_seconds),
        updated_at: timestamp(service.updated_at_seconds),
        id: service.id,
        name: service.name,
        ports: service.ports,
        version: service.version,
        stack: service.stack,
        update_state: service.update_state,
        update_message: service.update_message,
    }
}

fn task_response(task: TaskSummary) -> TaskResponse {
    TaskResponse {
        updated_at: timestamp(task.updated_at_seconds),
        id: task.id,
        slot: task.slot,
        swarm_node_id: task.node_id,
        state: task.state,
        desired_state: task.desired_state,
        error: task.error,
        message: task.message,
        container_id: task.container_id,
    }
}

fn quorum_unavailable() -> Response {
    let mut response = (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::CACHE_CONTROL, "no-store")],
        Json(QuorumUnavailableResponse {
            error: "quorum_unavailable".to_owned(),
            retry_after_seconds: QUORUM_RETRY_SECONDS,
        }),
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::RETRY_AFTER, HeaderValue::from(QUORUM_RETRY_SECONDS));
    response
}

pub(super) fn manager_read_error(error: ManagerError) -> Response {
    match error {
        ManagerError::Unavailable | ManagerError::QuorumUnavailable | ManagerError::Cancelled => {
            quorum_unavailable()
        }
        ManagerError::NotAManager => {
            admission_error(StatusCode::SERVICE_UNAVAILABLE, "not_a_manager")
        }
        _ => admission_error(StatusCode::SERVICE_UNAVAILABLE, "manager_unavailable"),
    }
}

pub(super) fn manager(context: &AppContext) -> Result<SharedManagerEngine, Box<Response>> {
    context
        .shared_store
        .get::<SharedManagerEngine>()
        .ok_or_else(|| {
            Box::new(admission_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "not_a_manager",
            ))
        })
}

pub(super) fn shutdown_token(context: &AppContext) -> CancellationToken {
    context
        .shared_store
        .get::<crate::server::ForceCancellation>()
        .map_or_else(CancellationToken::new, |force| force.0)
}

#[utoipa::path(
    get,
    path = "/api/v1/services",
    operation_id = "list_services",
    params(("stack" = Option<String>, Query, description = "Only this stack's services")),
    responses(
        (status = 200, description = "Every service with running replicas counted from one tasks call", body = ServiceListResponse),
        (status = 400, description = "`invalid_request` naming the query field", body = super::admission::FieldErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 503, description = "`quorum_unavailable` (retry after `retryAfterSeconds`) or `not_a_manager`", body = QuorumUnavailableResponse)
    )
)]
pub(crate) async fn index(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
    RawQuery(query): RawQuery,
) -> Response {
    let mut stack = None;
    for (key, value) in url::form_urlencoded::parse(query.as_deref().unwrap_or_default().as_bytes())
    {
        let valid =
            key == "stack" && stack.is_none() && kontainer_engine::is_valid_network_name(&value);
        if !valid {
            return field_error(&key, "invalid");
        }
        stack = Some(value.into_owned());
    }
    let manager = match manager(&context) {
        Ok(manager) => manager,
        Err(response) => return *response,
    };
    match list_services::execute(
        manager.0.as_ref(),
        stack.as_deref(),
        &shutdown_token(&context),
    )
    .await
    {
        Ok(rows) => json_no_store(
            StatusCode::OK,
            ServiceListResponse {
                items: rows.into_iter().map(service_response).collect(),
                snapshot_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            },
        ),
        Err(error) => manager_read_error(error),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/services/{serviceId}",
    operation_id = "get_service",
    params(("serviceId" = String, Path, description = "Swarm service id")),
    responses(
        (status = 200, description = "The service, its newest tasks, and the newest task error", body = ServiceDetailResponse),
        (status = 400, description = "`invalid_request`", body = super::admission::FieldErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 404, description = "No such service", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "`quorum_unavailable` or `not_a_manager`", body = QuorumUnavailableResponse)
    )
)]
pub(crate) async fn show(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
    Path(service_id): Path<String>,
) -> Response {
    if !is_valid_swarm_id(&service_id) {
        return field_error("serviceId", "invalid");
    }
    let manager = match manager(&context) {
        Ok(manager) => manager,
        Err(response) => return *response,
    };
    match get_service::execute(manager.0.as_ref(), &service_id, &shutdown_token(&context)).await {
        Ok(Some(detail)) => json_no_store(
            StatusCode::OK,
            ServiceDetailResponse {
                service: service_response(detail.row),
                tasks: detail
                    .tasks
                    .into_iter()
                    .take(MAX_TASKS)
                    .map(task_response)
                    .collect(),
                latest_error: detail.latest_error,
            },
        ),
        Ok(None) => admission_error(StatusCode::NOT_FOUND, "not_found"),
        Err(error) => manager_read_error(error),
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceActionRequest {
    /// The service version the change is based on; a newer one is 409 `version_conflict`.
    pub version: u64,
    /// Scale only: 0–1000.
    #[serde(default)]
    pub replicas: Option<u64>,
}

#[utoipa::path(
    post,
    path = "/api/v1/services/{serviceId}/actions/{action}",
    operation_id = "service_action",
    params(
        ("serviceId" = String, Path, description = "Swarm service id"),
        ("action" = String, Path, description = "`scale`, `restart`, or `rollback`"),
        ("Idempotency-Key" = String, Header, description = "16–64 printable ASCII characters, one per user gesture")
    ),
    request_body = ServiceActionRequest,
    responses(
        (status = 202, description = "Admitted as one `service.scale`, `service.restart`, or `service.rollback` operation that waits for convergence", body = super::admission::AdmittedResponse),
        (status = 400, description = "`idempotency_key_required`, or `invalid_request` naming the field", body = super::admission::FieldErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 404, description = "No such service or action", body = super::admission::AdmissionErrorResponse),
        (status = 409, description = "`version_conflict` or `idempotency_conflict`", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "`quorum_unavailable` (nothing admitted) or `not_a_manager`", body = QuorumUnavailableResponse)
    )
)]
pub(crate) async fn action(
    State(context): State<AppContext>,
    principal: PrincipalContext,
    Path((service_id, action)): Path<(String, String)>,
    key: IdempotencyKey,
    AdmittingRequestId(request_id): AdmittingRequestId,
    body: axum::body::Bytes,
) -> Response {
    let (action, kind) = match action.as_str() {
        "scale" => (ServiceAction::Scale, OperationKind::ServiceScale),
        "restart" => (ServiceAction::Restart, OperationKind::ServiceRestart),
        "rollback" => (ServiceAction::Rollback, OperationKind::ServiceRollback),
        _ => return admission_error(StatusCode::NOT_FOUND, "not_found"),
    };
    if !is_valid_swarm_id(&service_id) {
        return field_error("serviceId", "invalid");
    }
    let Ok(request) = serde_json::from_slice::<ServiceActionRequest>(&body) else {
        return field_error("body", "invalid");
    };
    match (action, request.replicas) {
        (ServiceAction::Scale, Some(replicas)) if replicas <= MAX_REPLICAS => {}
        (ServiceAction::Scale, _) => return field_error("replicas", "invalid"),
        (_, Some(_)) => return field_error("replicas", "not_allowed"),
        (_, None) => {}
    }
    let scope = format!("/services/{service_id}");
    if let Some(replay) = replay_existing(
        &context,
        &principal,
        &scope,
        &key,
        kind,
        &serde_json::json!({
            "serviceId": service_id,
            "version": request.version,
            "replicas": request.replicas,
        }),
    )
    .await
    {
        return replay;
    }
    let manager = match manager(&context) {
        Ok(manager) => manager,
        Err(response) => return *response,
    };
    // One read before admission: no quorum means no operation, and a stale version is refused
    // here rather than inside the executor.
    let rows =
        match list_services::execute(manager.0.as_ref(), None, &shutdown_token(&context)).await {
            Ok(rows) => rows,
            Err(error) => return manager_read_error(error),
        };
    let Some(row) = rows.into_iter().find(|row| row.service.id == service_id) else {
        return admission_error(StatusCode::NOT_FOUND, "not_found");
    };
    if row.service.version != request.version {
        return admission_error(StatusCode::CONFLICT, "version_conflict");
    }
    if action == ServiceAction::Scale && row.service.mode == ServiceMode::Global {
        return field_error("replicas", "global_service");
    }
    let payload = ServiceActionPayload {
        service_id,
        name: row.service.name,
        version: request.version,
        replicas: request.replicas,
    };
    admit(
        &context,
        Admission {
            principal,
            key,
            request_id,
            scope: &scope,
            kind,
            payload: serde_json::to_value(payload).unwrap_or_default(),
        },
    )
    .await
}

pub fn routes() -> Routes {
    Routes::new()
        .get("/services", index)
        .get("/services/{serviceId}", show)
        .add("/services/{serviceId}/actions/{action}", post(action))
}
