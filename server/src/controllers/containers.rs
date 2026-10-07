#![forbid(unsafe_code)]

//! Containers across the cluster and their lifecycle actions.
//!
//! The list fans out to member nodes and merges their answers with an outcome per node; actions
//! are admitted as durable operations. A node id selects a membership row, never an address.

use axum::{
    extract::{Path, RawQuery, State},
    http::StatusCode,
    response::Response,
};
use chrono::{DateTime, SecondsFormat, Utc};
use loco_rs::{app::AppContext, controller::Routes, prelude::post};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use utoipa::ToSchema;

use super::{
    admission::{Admission, AdmittingRequestId, IdempotencyKey, admission_error, admit},
    nodes::json_no_store,
};
use crate::{
    auth::domain::PrincipalContext,
    containers::{
        actions::list_containers::{self, ListContainersError},
        domain::{
            BulkActionPayload, ContainerActionPayload, ContainerItem, MAX_STOP_TIMEOUT_SECONDS,
            action_kind,
        },
    },
    nodes::{
        agents::SharedAgentDirectory,
        entities::node,
        fan_out::{FanOutResponse, NodeOutcomeResponse},
    },
    operations::{
        bulk::{BulkTarget, MAX_BULK_TARGETS},
        domain::OperationKind,
    },
    rpc::{AgentContainerAction, AgentContainerState, client::is_valid_container_id},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContainerStateResponse {
    Unknown,
    Created,
    Running,
    Paused,
    Restarting,
    Removing,
    Exited,
    Dead,
    Stopping,
}

impl From<AgentContainerState> for ContainerStateResponse {
    fn from(state: AgentContainerState) -> Self {
        match state {
            AgentContainerState::Unknown => Self::Unknown,
            AgentContainerState::Created => Self::Created,
            AgentContainerState::Running => Self::Running,
            AgentContainerState::Paused => Self::Paused,
            AgentContainerState::Restarting => Self::Restarting,
            AgentContainerState::Removing => Self::Removing,
            AgentContainerState::Exited => Self::Exited,
            AgentContainerState::Dead => Self::Dead,
            AgentContainerState::Stopping => Self::Stopping,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContainerResponse {
    pub node_id: String,
    /// The node's hostname.
    pub node_name: String,
    /// Full lowercase-hex daemon container ID.
    pub container_id: String,
    pub name: String,
    pub image: String,
    pub state: ContainerStateResponse,
    /// Bounded daemon status text, e.g. `Up 3 minutes`.
    pub status: String,
    pub created_at: String,
    #[schema(required = true)]
    pub started_at: Option<String>,
    #[schema(required = true)]
    pub finished_at: Option<String>,
    /// Published ports as `host:container/proto`.
    pub ports: Vec<String>,
    #[schema(required = true)]
    pub service_name: Option<String>,
    #[schema(required = true)]
    pub stack_namespace: Option<String>,
    #[schema(required = true)]
    pub task_id: Option<String>,
    /// From the node's last sample; null when not running.
    #[schema(required = true)]
    pub cpu_percent: Option<f64>,
    #[schema(required = true)]
    pub memory_used_bytes: Option<u64>,
    #[schema(required = true)]
    pub memory_limit_bytes: Option<u64>,
    /// Networks it has endpoints on, by name (at most 16).
    pub networks: Vec<String>,
}

/// `{ items, nodes, snapshotAt }`: merged containers and one outcome per member node.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContainerListResponse {
    #[schema(max_items = 2000)]
    pub items: Vec<ContainerResponse>,
    pub nodes: Vec<NodeOutcomeResponse>,
    pub snapshot_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TooManyContainersResponse {
    pub error: String,
    pub hint: String,
}

fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn container_response(item: ContainerItem) -> ContainerResponse {
    ContainerResponse {
        node_id: item.node_id,
        node_name: item.node_name,
        container_id: item.container_id,
        name: item.name,
        image: item.image,
        state: item.state.into(),
        status: item.status,
        created_at: timestamp(item.created_at),
        started_at: item.started_at.map(timestamp),
        finished_at: item.finished_at.map(timestamp),
        ports: item.ports,
        service_name: item.service_name,
        stack_namespace: item.stack_namespace,
        task_id: item.task_id,
        cpu_percent: item.cpu_percent,
        memory_used_bytes: item.memory_used_bytes,
        memory_limit_bytes: item.memory_limit_bytes,
        networks: item.networks,
    }
}

fn valid_node_id(value: &str) -> bool {
    kontainer_rpc_auth::valid_identity_segment(value)
}

/// Only `node`, at most once.
fn parse_node_filter(raw: Option<&str>) -> Result<Option<String>, ()> {
    let mut node = None;
    for (key, value) in url::form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
        if key != "node" || node.is_some() || !valid_node_id(&value) {
            return Err(());
        }
        node = Some(value.into_owned());
    }
    Ok(node)
}

#[utoipa::path(
    get,
    path = "/api/v1/containers",
    operation_id = "list_containers",
    params(("node" = Option<String>, Query, description = "Only this member node")),
    responses(
        (status = 200, description = "Containers on every answering node, and one outcome per member node", body = ContainerListResponse),
        (status = 400, description = "Invalid query", body = super::admission::AdmissionErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 404, description = "No such member node", body = super::admission::AdmissionErrorResponse),
        (status = 413, description = "`too_many_containers`: more than 2,000; filter by node", body = TooManyContainersResponse),
        (status = 503, description = "Agent RPC not configured, persistence unavailable, or controller draining", body = super::admission::AdmissionErrorResponse)
    )
)]
pub(crate) async fn index(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
    RawQuery(query): RawQuery,
) -> Response {
    let Ok(node_filter) = parse_node_filter(query.as_deref()) else {
        return admission_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let Some(directory) = context.shared_store.get::<SharedAgentDirectory>() else {
        return admission_error(StatusCode::SERVICE_UNAVAILABLE, "agent_unavailable");
    };
    let shutdown = context
        .shared_store
        .get::<crate::server::ForceCancellation>()
        .map_or_else(CancellationToken::new, |force| force.0);
    // Awaited in place: dropping the request drops every node call with it.
    match list_containers::execute(
        &context.db,
        directory.0.as_ref(),
        node_filter.as_deref(),
        &shutdown,
    )
    .await
    {
        Ok(list) => {
            let outcomes = FanOutResponse::<ContainerResponse>::from_outcomes(
                list.outcomes,
                list.snapshot_at,
                |_, ()| Vec::new(),
            );
            json_no_store(
                StatusCode::OK,
                ContainerListResponse {
                    items: list.items.into_iter().map(container_response).collect(),
                    nodes: outcomes.nodes,
                    snapshot_at: outcomes.snapshot_at,
                },
            )
        }
        Err(ListContainersError::UnknownNode) => {
            admission_error(StatusCode::NOT_FOUND, "not_found")
        }
        Err(ListContainersError::TooMany) => json_no_store(
            StatusCode::PAYLOAD_TOO_LARGE,
            TooManyContainersResponse {
                error: "too_many_containers".to_owned(),
                hint: "More than 2,000 containers; list one node with ?node=<id>.".to_owned(),
            },
        ),
        Err(ListContainersError::Persistence(_)) => {
            admission_error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable")
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContainerActionRequest {
    /// Remove only: kill a running container first.
    #[serde(default)]
    pub force: bool,
    /// Stop and restart only: grace before kill, 0–600 (default 10).
    #[serde(default)]
    pub timeout_seconds: Option<u32>,
}

/// An empty body means the defaults; otherwise the strict JSON shape.
fn parse_action_body(body: &[u8]) -> Result<ContainerActionRequest, ()> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(ContainerActionRequest::default());
    }
    let request: ContainerActionRequest = serde_json::from_slice(body).map_err(|_| ())?;
    if request
        .timeout_seconds
        .is_some_and(|seconds| seconds > MAX_STOP_TIMEOUT_SECONDS)
    {
        return Err(());
    }
    Ok(request)
}

/// `None` when membership could not be read.
async fn is_member(context: &AppContext, node_id: &str) -> Option<bool> {
    use sea_orm::EntityTrait;
    node::Entity::find_by_id(node_id.to_owned())
        .one(&context.db)
        .await
        .ok()
        .map(|row| row.is_some())
}

#[utoipa::path(
    post,
    path = "/api/v1/containers/{nodeId}/{containerId}/actions/{action}",
    operation_id = "container_action",
    params(
        ("nodeId" = String, Path, description = "Member node id"),
        ("containerId" = String, Path, description = "Full container id"),
        ("action" = String, Path, description = "start, stop, restart, pause, unpause, or remove"),
        ("Idempotency-Key" = String, Header, description = "16–64 printable ASCII characters, one per user gesture")
    ),
    request_body(content = Option<ContainerActionRequest>, description = "Optional; empty means defaults"),
    responses(
        (status = 202, description = "Admitted; poll the operation", body = super::admission::AdmittedResponse),
        (status = 400, description = "`idempotency_key_required` or `invalid_request`", body = super::admission::AdmissionErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 404, description = "No such member node", body = super::admission::AdmissionErrorResponse),
        (status = 409, description = "`idempotency_conflict`", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "Persistence unavailable or controller draining")
    )
)]
pub(crate) async fn action(
    State(context): State<AppContext>,
    principal: PrincipalContext,
    Path((node_id, container_id, action)): Path<(String, String, String)>,
    key: IdempotencyKey,
    AdmittingRequestId(request_id): AdmittingRequestId,
    body: axum::body::Bytes,
) -> Response {
    let (Some(action), true, true, Ok(request)) = (
        AgentContainerAction::parse(&action),
        valid_node_id(&node_id),
        is_valid_container_id(&container_id),
        parse_action_body(&body),
    ) else {
        return admission_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    match is_member(&context, &node_id).await {
        Some(true) => {}
        Some(false) => return admission_error(StatusCode::NOT_FOUND, "not_found"),
        None => {
            return admission_error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable");
        }
    }
    let scope = format!(
        "/containers/{node_id}/{container_id}/actions/{}",
        action.as_str()
    );
    let payload = ContainerActionPayload {
        node_id,
        container_id,
        action: action.as_str().to_owned(),
        force: request.force && action == AgentContainerAction::Remove,
        timeout_seconds: request.timeout_seconds.filter(|_| {
            matches!(
                action,
                AgentContainerAction::Stop | AgentContainerAction::Restart
            )
        }),
    };
    admit(
        &context,
        Admission {
            principal,
            key,
            request_id,
            scope: &scope,
            kind: action_kind(action),
            payload: serde_json::to_value(payload).unwrap_or_default(),
        },
    )
    .await
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BulkActionRequest {
    /// start, stop, restart, pause, unpause, or remove.
    pub action: String,
    /// 1–200 containers on any member nodes.
    #[schema(value_type = Vec<Object>, max_items = 200)]
    pub targets: Vec<BulkTarget>,
    /// Remove only: kill running containers first.
    #[serde(default)]
    pub force: bool,
}

#[utoipa::path(
    post,
    path = "/api/v1/containers/actions/bulk",
    operation_id = "containers_bulk_action",
    params(("Idempotency-Key" = String, Header, description = "16–64 printable ASCII characters, one per user gesture")),
    request_body = BulkActionRequest,
    responses(
        (status = 202, description = "Admitted as one `containers.bulk` operation", body = super::admission::AdmittedResponse),
        (status = 400, description = "`idempotency_key_required` or `invalid_request`", body = super::admission::AdmissionErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 409, description = "`idempotency_conflict`", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "Persistence unavailable or controller draining")
    )
)]
pub(crate) async fn bulk(
    State(context): State<AppContext>,
    principal: PrincipalContext,
    key: IdempotencyKey,
    AdmittingRequestId(request_id): AdmittingRequestId,
    body: axum::body::Bytes,
) -> Response {
    let Ok(request) = serde_json::from_slice::<BulkActionRequest>(&body) else {
        return admission_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let Some(action) = AgentContainerAction::parse(&request.action) else {
        return admission_error(StatusCode::BAD_REQUEST, "invalid_request");
    };
    let mut seen = std::collections::HashSet::new();
    let valid = !request.targets.is_empty()
        && request.targets.len() <= MAX_BULK_TARGETS
        && request.targets.iter().all(|target| {
            valid_node_id(&target.node_id)
                && is_valid_container_id(&target.container_id)
                && seen.insert((target.node_id.clone(), target.container_id.clone()))
        });
    if !valid {
        return admission_error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    let payload = BulkActionPayload {
        action: action.as_str().to_owned(),
        targets: request.targets,
        force: request.force && action == AgentContainerAction::Remove,
    };
    admit(
        &context,
        Admission {
            principal,
            key,
            request_id,
            scope: "/containers/actions/bulk",
            kind: OperationKind::ContainersBulk,
            payload: serde_json::to_value(payload).unwrap_or_default(),
        },
    )
    .await
}

pub fn routes() -> Routes {
    Routes::new()
        .get("/containers", index)
        .add("/containers/actions/bulk", post(bulk))
        .add(
            "/containers/{nodeId}/{containerId}/actions/{action}",
            post(action),
        )
}

#[cfg(test)]
mod tests {
    use super::{parse_action_body, parse_node_filter};

    #[test]
    fn queries_and_bodies_are_strict() {
        assert_eq!(parse_node_filter(None), Ok(None));
        assert_eq!(
            parse_node_filter(Some("node=worker-1")),
            Ok(Some("worker-1".to_owned()))
        );
        for bad in ["node=a/b", "node=x&node=y", "other=1", "node="] {
            assert!(parse_node_filter(Some(bad)).is_err(), "{bad}");
        }
        assert!(parse_action_body(b"").is_ok());
        assert!(parse_action_body(b"  ").is_ok());
        let parsed = parse_action_body(br#"{"force":true,"timeoutSeconds":30}"#).unwrap();
        assert!(parsed.force);
        assert_eq!(parsed.timeout_seconds, Some(30));
        for bad in [
            &br#"{"timeoutSeconds":601}"#[..],
            br#"{"extra":1}"#,
            b"not json",
        ] {
            assert!(parse_action_body(bad).is_err());
        }
    }
}
