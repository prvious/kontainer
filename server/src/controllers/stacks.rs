#![forbid(unsafe_code)]

//! Stacks derived from service labels: the list, the removal plan, and removal.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Response,
};
use chrono::{DateTime, SecondsFormat, Utc};
use kontainer_engine::is_valid_network_name;
use loco_rs::{app::AppContext, controller::Routes, prelude::delete};
use serde::Serialize;
use utoipa::ToSchema;

use super::{
    admission::{
        Admission, AdmittingRequestId, IdempotencyKey, admission_error, admit, field_error,
        replay_existing,
    },
    nodes::json_no_store,
    services::{manager, manager_read_error, shutdown_token},
};
use crate::{
    auth::domain::PrincipalContext,
    nodes::agents::SharedAgentDirectory,
    operations::domain::OperationKind,
    stacks::{
        actions::{list_stacks, removal_plan},
        domain::{RemovalPlan, StackRemovePayload, StackRow},
    },
};

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StackResponse {
    pub name: String,
    /// `running` (every service converged), `partial`, or `stopped` (no task runs).
    pub status: String,
    pub services: u32,
    /// Tasks running and desired, summed over the stack's services.
    pub running_containers: u64,
    pub desired_containers: u64,
    /// Summed from the stack's running containers' last samples.
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<u64>,
    /// The newest service update.
    pub last_deployed: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StackListResponse {
    pub items: Vec<StackResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StackPlanResponse {
    pub services: Vec<String>,
    pub networks: Vec<String>,
    /// Named volumes the services mount; removal leaves them.
    pub volumes_retained: Vec<String>,
}

fn stack_response(row: StackRow) -> StackResponse {
    StackResponse {
        status: row.status.as_str().to_owned(),
        last_deployed: row
            .last_deployed_seconds
            .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0))
            .map(|at| at.to_rfc3339_opts(SecondsFormat::Millis, true)),
        name: row.name,
        services: row.services,
        running_containers: row.running,
        desired_containers: row.desired,
        cpu_percent: row.cpu_percent,
        memory_bytes: row.memory_bytes,
    }
}

fn plan_response(plan: RemovalPlan) -> StackPlanResponse {
    StackPlanResponse {
        services: plan.services,
        networks: plan.networks,
        volumes_retained: plan.volumes_retained,
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/stacks",
    operation_id = "list_stacks",
    responses(
        (status = 200, description = "Stacks grouped from service labels, with summed task counts and container figures", body = StackListResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 503, description = "`quorum_unavailable` or `not_a_manager`", body = super::services::QuorumUnavailableResponse)
    )
)]
pub(crate) async fn index(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
) -> Response {
    let manager = match manager(&context) {
        Ok(manager) => manager,
        Err(response) => return *response,
    };
    let directory = context.shared_store.get::<SharedAgentDirectory>();
    match list_stacks::execute(
        &context.db,
        manager.0.as_ref(),
        directory.as_ref().map(|directory| directory.0.as_ref()),
        &shutdown_token(&context),
    )
    .await
    {
        Ok(rows) => json_no_store(
            StatusCode::OK,
            StackListResponse {
                items: rows.into_iter().map(stack_response).collect(),
            },
        ),
        Err(error) => manager_read_error(error),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/stacks/{name}/plan",
    operation_id = "stack_removal_plan",
    params(("name" = String, Path, description = "Stack name")),
    responses(
        (status = 200, description = "What removing the stack takes away and what it retains", body = StackPlanResponse),
        (status = 400, description = "`invalid_request`", body = super::admission::FieldErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 404, description = "No service carries this stack name", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "`quorum_unavailable` or `not_a_manager`", body = super::services::QuorumUnavailableResponse)
    )
)]
pub(crate) async fn plan(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
    Path(name): Path<String>,
) -> Response {
    // Stack names follow Docker's network-name rule.
    if !is_valid_network_name(&name) {
        return field_error("name", "invalid");
    }
    let manager = match manager(&context) {
        Ok(manager) => manager,
        Err(response) => return *response,
    };
    match removal_plan::execute(manager.0.as_ref(), &name, &shutdown_token(&context)).await {
        Ok(Some(planned)) => json_no_store(StatusCode::OK, plan_response(planned.plan)),
        Ok(None) => admission_error(StatusCode::NOT_FOUND, "not_found"),
        Err(error) => manager_read_error(error),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/stacks/{name}",
    operation_id = "remove_stack",
    params(
        ("name" = String, Path, description = "Stack name"),
        ("Idempotency-Key" = String, Header, description = "16–64 printable ASCII characters, one per user gesture")
    ),
    responses(
        (status = 202, description = "Admitted as one `stack.remove` operation: services, then their networks; volumes are kept", body = super::admission::AdmittedResponse),
        (status = 400, description = "`idempotency_key_required` or `invalid_request`", body = super::admission::FieldErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 404, description = "No service carries this stack name", body = super::admission::AdmissionErrorResponse),
        (status = 409, description = "`idempotency_conflict`", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "`quorum_unavailable` (nothing admitted) or `not_a_manager`", body = super::services::QuorumUnavailableResponse)
    )
)]
pub(crate) async fn remove(
    State(context): State<AppContext>,
    principal: PrincipalContext,
    Path(name): Path<String>,
    key: IdempotencyKey,
    AdmittingRequestId(request_id): AdmittingRequestId,
) -> Response {
    if !is_valid_network_name(&name) {
        return field_error("name", "invalid");
    }
    let scope = format!("/stacks/{name}");
    let payload =
        serde_json::to_value(StackRemovePayload { name: name.clone() }).unwrap_or_default();
    if let Some(replay) = replay_existing(
        &context,
        &principal,
        &scope,
        &key,
        OperationKind::StackRemove,
        &payload,
    )
    .await
    {
        return replay;
    }
    let manager = match manager(&context) {
        Ok(manager) => manager,
        Err(response) => return *response,
    };
    // The plan read doubles as the quorum check: without one, nothing is admitted.
    match removal_plan::execute(manager.0.as_ref(), &name, &shutdown_token(&context)).await {
        Ok(Some(_)) => {}
        Ok(None) => return admission_error(StatusCode::NOT_FOUND, "not_found"),
        Err(error) => return manager_read_error(error),
    }
    admit(
        &context,
        Admission {
            principal,
            key,
            request_id,
            scope: &scope,
            kind: OperationKind::StackRemove,
            payload,
        },
    )
    .await
}

pub fn routes() -> Routes {
    Routes::new()
        .get("/stacks", index)
        .get("/stacks/{name}/plan", plan)
        .add("/stacks/{name}", delete(remove))
}
