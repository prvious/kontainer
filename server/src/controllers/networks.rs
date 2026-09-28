#![forbid(unsafe_code)]

//! Networks across the cluster: the merged list, one network's attached containers, create, and
//! remove. Swarm-scope changes go to the controller's manager engine, local ones to the node.

use axum::{
    extract::{Path, RawQuery, State},
    http::StatusCode,
    response::Response,
};
use chrono::{DateTime, SecondsFormat, Utc};
use kontainer_engine::{
    is_builtin_network, is_valid_cidr, is_valid_network_id, is_valid_network_name,
};
use loco_rs::{
    app::AppContext,
    controller::Routes,
    prelude::{delete, post},
};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use utoipa::ToSchema;

use super::{
    admission::{
        Admission, AdmittingRequestId, IdempotencyKey, admission_error, admit, field_error,
    },
    images::all_members,
    nodes::json_no_store,
};
use crate::{
    auth::domain::PrincipalContext,
    networks::{
        actions::{
            inspect_network::{self, InspectNetworkError},
            list_networks::{self, NetworkList},
        },
        domain::{CreateNetworkPayload, NetworkRow, RemoveNetworkPayload},
    },
    nodes::{
        agents::SharedAgentDirectory,
        fan_out::{FanOutResponse, NodeOutcomeResponse},
    },
    operations::domain::OperationKind,
    swarm::engine::SharedManagerEngine,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NetworkResponse {
    pub id: String,
    pub name: String,
    pub driver: String,
    /// `swarm` (one row for the cluster) or `local` (one row per node).
    pub scope: String,
    pub subnets: Vec<String>,
    /// Running containers attached, summed across nodes.
    pub containers: u32,
    pub created_at: Option<String>,
    pub attachable: bool,
    pub internal: bool,
    /// Docker's own networks (`host`, `bridge`, `none`, `docker_gwbridge`, `ingress`).
    pub builtin: bool,
    /// Local: its node. Swarm: the nodes with endpoints on it.
    pub node_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NetworkListResponse {
    pub items: Vec<NetworkResponse>,
    pub nodes: Vec<NodeOutcomeResponse>,
    /// `ok`, `not_a_manager`, or `unavailable`: whether swarm-scope rows came from the manager.
    pub manager: String,
    pub snapshot_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttachedContainerResponse {
    pub node_id: String,
    pub container_id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NetworkContainersResponse {
    pub containers: Vec<AttachedContainerResponse>,
}

fn network_response(row: NetworkRow) -> NetworkResponse {
    NetworkResponse {
        created_at: row.created_at_seconds.and_then(|seconds| {
            DateTime::<Utc>::from_timestamp(seconds, 0)
                .map(|at| at.to_rfc3339_opts(SecondsFormat::Millis, true))
        }),
        id: row.id,
        name: row.name,
        driver: row.driver,
        scope: row.scope,
        subnets: row.subnets,
        containers: row.containers,
        attachable: row.attachable,
        internal: row.internal,
        builtin: row.builtin,
        node_ids: row.node_ids,
    }
}

fn shutdown_token(context: &AppContext) -> CancellationToken {
    context
        .shared_store
        .get::<crate::server::ForceCancellation>()
        .map_or_else(CancellationToken::new, |force| force.0)
}

async fn current_list(context: &AppContext) -> Result<NetworkList, Box<Response>> {
    let Some(directory) = context.shared_store.get::<SharedAgentDirectory>() else {
        return Err(Box::new(admission_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "agent_unavailable",
        )));
    };
    let manager = context.shared_store.get::<SharedManagerEngine>();
    list_networks::execute(
        &context.db,
        directory.0.as_ref(),
        manager.as_ref().map(|manager| &manager.0),
        &shutdown_token(context),
    )
    .await
    .map_err(|_| {
        Box::new(admission_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "persistence_unavailable",
        ))
    })
}

/// An optional `node` query parameter, at most once.
/// The offending field's name when it isn't.
fn node_query(raw: Option<&str>) -> Result<Option<String>, String> {
    let mut node = None;
    for (key, value) in url::form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
        if key != "node" || node.is_some() || !kontainer_rpc_auth::valid_identity_segment(&value) {
            return Err(key.into_owned());
        }
        node = Some(value.into_owned());
    }
    Ok(node)
}

#[utoipa::path(
    get,
    path = "/api/v1/networks",
    operation_id = "list_networks",
    responses(
        (status = 200, description = "Swarm-scope networks once, local ones per node, and one outcome per member", body = NetworkListResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 503, description = "Agent RPC not configured, persistence unavailable, or controller draining", body = super::admission::AdmissionErrorResponse)
    ),
    security(("access_session" = []))
)]
pub(crate) async fn index(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
) -> Response {
    let list = match current_list(&context).await {
        Ok(list) => list,
        Err(response) => return *response,
    };
    let outcomes = FanOutResponse::<NetworkResponse>::from_outcomes(
        list.outcomes,
        list.snapshot_at,
        |_, ()| Vec::new(),
    );
    json_no_store(
        StatusCode::OK,
        NetworkListResponse {
            items: list.items.into_iter().map(network_response).collect(),
            nodes: outcomes.nodes,
            manager: list.manager.as_str().to_owned(),
            snapshot_at: outcomes.snapshot_at,
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/networks/{networkId}",
    operation_id = "inspect_network",
    params(
        ("networkId" = String, Path, description = "Docker network id"),
        ("node" = Option<String>, Query, description = "The node of a local network; omit for a swarm-scope one")
    ),
    responses(
        (status = 200, description = "The containers attached, with their nodes", body = NetworkContainersResponse),
        (status = 400, description = "`invalid_request` naming the field", body = super::admission::FieldErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 404, description = "No such member node, or the node doesn't have it", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "Agent RPC not configured, agent or persistence unavailable", body = super::admission::AdmissionErrorResponse)
    ),
    security(("access_session" = []))
)]
pub(crate) async fn show(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
    Path(network_id): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    if !is_valid_network_id(&network_id) {
        return field_error("networkId", "invalid");
    }
    let node = match node_query(query.as_deref()) {
        Ok(node) => node,
        Err(field) => return field_error(&field, "invalid"),
    };
    let Some(directory) = context.shared_store.get::<SharedAgentDirectory>() else {
        return admission_error(StatusCode::SERVICE_UNAVAILABLE, "agent_unavailable");
    };
    match inspect_network::execute(
        &context.db,
        directory.0.as_ref(),
        &network_id,
        node.as_deref(),
        &shutdown_token(&context),
    )
    .await
    {
        Ok(containers) => json_no_store(
            StatusCode::OK,
            NetworkContainersResponse {
                containers: containers
                    .into_iter()
                    .map(|container| AttachedContainerResponse {
                        node_id: container.node_id,
                        container_id: container.container_id,
                        name: container.name,
                    })
                    .collect(),
            },
        ),
        Err(InspectNetworkError::NotFound) => admission_error(StatusCode::NOT_FOUND, "not_found"),
        Err(InspectNetworkError::Persistence) => {
            admission_error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable")
        }
        Err(InspectNetworkError::Agent(_)) => {
            admission_error(StatusCode::SERVICE_UNAVAILABLE, "agent_unavailable")
        }
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateNetworkRequest {
    /// `[a-zA-Z0-9][a-zA-Z0-9_.-]*`, 1–128 characters, unique across the current list.
    pub name: String,
    /// `overlay` (swarm scope, on the manager) or `bridge` (local, on `nodeId`).
    pub driver: String,
    /// Overlay only: standalone containers may attach.
    #[serde(default)]
    pub attachable: bool,
    /// Required for `bridge`, refused for `overlay`.
    #[serde(default)]
    pub node_id: Option<String>,
    /// A canonical CIDR, e.g. `10.0.9.0/24`.
    #[serde(default)]
    pub subnet: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/v1/networks",
    operation_id = "create_network",
    params(("Idempotency-Key" = String, Header, description = "16–64 printable ASCII characters, one per user gesture")),
    request_body = CreateNetworkRequest,
    responses(
        (status = 202, description = "Admitted as one `network.create` operation", body = super::admission::AdmittedResponse),
        (status = 400, description = "`idempotency_key_required`, or `invalid_request` naming `name`, `driver`, `nodeId`, or `subnet`", body = super::admission::FieldErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 409, description = "`idempotency_conflict`", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "`not_a_manager` for an overlay without a Swarm manager, persistence unavailable, or draining", body = super::admission::AdmissionErrorResponse)
    ),
    security(("access_session" = []))
)]
pub(crate) async fn create(
    State(context): State<AppContext>,
    principal: PrincipalContext,
    key: IdempotencyKey,
    AdmittingRequestId(request_id): AdmittingRequestId,
    body: axum::body::Bytes,
) -> Response {
    let Ok(request) = serde_json::from_slice::<CreateNetworkRequest>(&body) else {
        return field_error("body", "invalid");
    };
    if !is_valid_network_name(&request.name) {
        return field_error("name", "invalid");
    }
    if is_builtin_network(&request.name) {
        return field_error("name", "reserved");
    }
    if request
        .subnet
        .as_deref()
        .is_some_and(|subnet| !is_valid_cidr(subnet))
    {
        return field_error("subnet", "invalid");
    }
    match (request.driver.as_str(), request.node_id.as_deref()) {
        ("overlay", None) => {
            if context.shared_store.get::<SharedManagerEngine>().is_none() {
                return admission_error(StatusCode::SERVICE_UNAVAILABLE, "not_a_manager");
            }
        }
        ("overlay", Some(_)) => return field_error("nodeId", "not_allowed"),
        ("bridge", Some(node_id)) => match all_members(&context, &[node_id.to_owned()]).await {
            Some(true) => {}
            Some(false) => return field_error("nodeId", "invalid"),
            None => {
                return admission_error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable");
            }
        },
        ("bridge", None) => return field_error("nodeId", "required"),
        _ => return field_error("driver", "unsupported"),
    }
    let list = match current_list(&context).await {
        Ok(list) => list,
        Err(response) => return *response,
    };
    if list.items.iter().any(|row| row.name == request.name) {
        return field_error("name", "taken");
    }
    let payload = CreateNetworkPayload {
        attachable: request.driver == "overlay" && request.attachable,
        name: request.name,
        driver: request.driver,
        node_id: request.node_id,
        subnet: request.subnet,
    };
    admit(
        &context,
        Admission {
            principal,
            key,
            request_id,
            scope: "/networks",
            kind: OperationKind::NetworkCreate,
            payload: serde_json::to_value(payload).unwrap_or_default(),
        },
    )
    .await
}

#[utoipa::path(
    delete,
    path = "/api/v1/networks/{networkId}",
    operation_id = "remove_network",
    params(
        ("networkId" = String, Path, description = "Docker network id"),
        ("node" = Option<String>, Query, description = "The node of a local network; omit for a swarm-scope one"),
        ("Idempotency-Key" = String, Header, description = "16–64 printable ASCII characters, one per user gesture")
    ),
    responses(
        (status = 202, description = "Admitted as one `network.remove` operation; attached containers fail it with `in_use` and their names", body = super::admission::AdmittedResponse),
        (status = 400, description = "`builtin_network`, `idempotency_key_required`, or `invalid_request`", body = super::admission::AdmissionErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 404, description = "No such network (on that node)", body = super::admission::AdmissionErrorResponse),
        (status = 409, description = "`idempotency_conflict`", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "`not_a_manager` for a swarm network without a Swarm manager, persistence unavailable, or draining", body = super::admission::AdmissionErrorResponse)
    ),
    security(("access_session" = []))
)]
pub(crate) async fn remove(
    State(context): State<AppContext>,
    principal: PrincipalContext,
    Path(network_id): Path<String>,
    RawQuery(query): RawQuery,
    key: IdempotencyKey,
    AdmittingRequestId(request_id): AdmittingRequestId,
) -> Response {
    if !is_valid_network_id(&network_id) {
        return field_error("networkId", "invalid");
    }
    let node = match node_query(query.as_deref()) {
        Ok(node) => node,
        Err(field) => return field_error(&field, "invalid"),
    };
    let list = match current_list(&context).await {
        Ok(list) => list,
        Err(response) => return *response,
    };
    let Some(row) = list.items.into_iter().find(|row| {
        row.id == network_id
            && match node.as_deref() {
                Some(node) => !row.is_swarm() && row.node_ids.iter().any(|id| id == node),
                None => row.is_swarm(),
            }
    }) else {
        return admission_error(StatusCode::NOT_FOUND, "not_found");
    };
    if row.builtin {
        return admission_error(StatusCode::BAD_REQUEST, "builtin_network");
    }
    if row.is_swarm() && context.shared_store.get::<SharedManagerEngine>().is_none() {
        return admission_error(StatusCode::SERVICE_UNAVAILABLE, "not_a_manager");
    }
    let scope = format!("/networks/{network_id}");
    let payload = RemoveNetworkPayload {
        network_id,
        name: row.name,
        node_id: node,
    };
    admit(
        &context,
        Admission {
            principal,
            key,
            request_id,
            scope: &scope,
            kind: OperationKind::NetworkRemove,
            payload: serde_json::to_value(payload).unwrap_or_default(),
        },
    )
    .await
}

pub fn routes() -> Routes {
    Routes::new()
        .get("/networks", index)
        .add("/networks", post(create))
        .get("/networks/{networkId}", show)
        .add("/networks/{networkId}", delete(remove))
}
