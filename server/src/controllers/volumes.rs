#![forbid(unsafe_code)]

//! Node-scoped volumes: the list (with the create dialog's conflict query), create, and remove.

use std::collections::BTreeMap;

use axum::{
    extract::{Path, RawQuery, State},
    http::StatusCode,
    response::Response,
};
use chrono::{DateTime, SecondsFormat, Utc};
use kontainer_engine::{are_valid_volume_labels, is_valid_volume_name};
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
    nodes::{
        agents::SharedAgentDirectory,
        fan_out::{FanOutResponse, NodeOutcomeResponse},
    },
    operations::domain::OperationKind,
    volumes::{
        actions::list_volumes::{self, ListVolumesError, VolumeFilter},
        domain::{CreateVolumePayload, LOCAL_DRIVER, RemoveVolumePayload, VolumeRow},
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VolumeResponse {
    pub node_id: String,
    pub name: String,
    pub driver: String,
    /// Host path of a `local` volume.
    pub mount_point: String,
    pub created_at: Option<String>,
    pub labels: BTreeMap<String, String>,
    /// Containers on the node (any state) that mount it.
    pub used_by: u32,
    /// Up to 16 of their names.
    pub users: Vec<String>,
    /// From the node's last disk sample; `null` until then or when the driver can't say.
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VolumeListResponse {
    pub items: Vec<VolumeResponse>,
    pub nodes: Vec<NodeOutcomeResponse>,
    pub snapshot_at: String,
}

fn volume_response(row: VolumeRow) -> VolumeResponse {
    VolumeResponse {
        created_at: row.created_at_seconds.and_then(|seconds| {
            DateTime::<Utc>::from_timestamp(seconds, 0)
                .map(|at| at.to_rfc3339_opts(SecondsFormat::Millis, true))
        }),
        node_id: row.node_id,
        name: row.name,
        driver: row.driver,
        mount_point: row.mountpoint,
        labels: row.labels,
        used_by: row.used_by,
        users: row.users,
        size_bytes: row.size_bytes,
    }
}

/// The list query: `node` and `name`, each at most once.
#[derive(Debug, Default)]
struct ListQuery {
    node: Option<String>,
    name: Option<String>,
}

/// The query field that failed and why.
struct QueryProblem {
    field: String,
    problem: &'static str,
}

fn parse_filter(raw: Option<&str>) -> Result<ListQuery, QueryProblem> {
    let mut query = ListQuery::default();
    for (key, value) in url::form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
        match key.as_ref() {
            "node"
                if query.node.is_none() && kontainer_rpc_auth::valid_identity_segment(&value) =>
            {
                query.node = Some(value.into_owned());
            }
            "name" if query.name.is_none() && is_valid_volume_name(&value) => {
                query.name = Some(value.into_owned());
            }
            field => {
                let problem = if matches!(field, "node" | "name") {
                    "invalid"
                } else {
                    "unknown"
                };
                return Err(QueryProblem {
                    field: field.to_owned(),
                    problem,
                });
            }
        }
    }
    Ok(query)
}

#[utoipa::path(
    get,
    path = "/api/v1/volumes",
    operation_id = "list_volumes",
    params(
        ("node" = Option<String>, Query, description = "Only this member node"),
        ("name" = Option<String>, Query, description = "Only volumes with this exact name (the create dialog's conflict check)")
    ),
    responses(
        (status = 200, description = "Volumes on every answering node, one row per node and name, and one outcome per member", body = VolumeListResponse),
        (status = 400, description = "`invalid_request` naming the query field", body = super::admission::FieldErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 404, description = "No such member node", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "Agent RPC not configured, persistence unavailable, or controller draining", body = super::admission::AdmissionErrorResponse)
    ),
    security(("access_session" = []))
)]
pub(crate) async fn index(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
    RawQuery(query): RawQuery,
) -> Response {
    let filter = match parse_filter(query.as_deref()) {
        Ok(filter) => filter,
        Err(QueryProblem { field, problem }) => return field_error(&field, problem),
    };
    let Some(directory) = context.shared_store.get::<SharedAgentDirectory>() else {
        return admission_error(StatusCode::SERVICE_UNAVAILABLE, "agent_unavailable");
    };
    let shutdown = context
        .shared_store
        .get::<crate::server::ForceCancellation>()
        .map_or_else(CancellationToken::new, |force| force.0);
    let volume_filter = VolumeFilter {
        node_id: filter.node.as_deref(),
        name: filter.name.as_deref(),
    };
    match list_volumes::execute(&context.db, directory.0.as_ref(), volume_filter, &shutdown).await {
        Ok(list) => {
            let outcomes = FanOutResponse::<VolumeResponse>::from_outcomes(
                list.outcomes,
                list.snapshot_at,
                |_, ()| Vec::new(),
            );
            json_no_store(
                StatusCode::OK,
                VolumeListResponse {
                    items: list.items.into_iter().map(volume_response).collect(),
                    nodes: outcomes.nodes,
                    snapshot_at: outcomes.snapshot_at,
                },
            )
        }
        Err(ListVolumesError::UnknownNode) => admission_error(StatusCode::NOT_FOUND, "not_found"),
        Err(ListVolumesError::Persistence(_)) => {
            admission_error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable")
        }
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateVolumeRequest {
    /// The member node that holds it; volumes are node-local.
    pub node_id: String,
    /// `[a-zA-Z0-9][a-zA-Z0-9_.-]+`, 2–255 characters.
    pub name: String,
    /// `local` only in this phase.
    pub driver: String,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
}

#[utoipa::path(
    post,
    path = "/api/v1/volumes",
    operation_id = "create_volume",
    params(("Idempotency-Key" = String, Header, description = "16–64 printable ASCII characters, one per user gesture")),
    request_body = CreateVolumeRequest,
    responses(
        (status = 202, description = "Admitted as one `volume.create` operation", body = super::admission::AdmittedResponse),
        (status = 400, description = "`idempotency_key_required`, or `invalid_request` naming `nodeId`, `name`, `driver`, or `labels`", body = super::admission::FieldErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 409, description = "`idempotency_conflict`", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "Persistence unavailable or controller draining")
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
    let Ok(request) = serde_json::from_slice::<CreateVolumeRequest>(&body) else {
        return field_error("body", "invalid");
    };
    if !is_valid_volume_name(&request.name) {
        return field_error("name", "invalid");
    }
    if request.driver != LOCAL_DRIVER {
        return field_error("driver", "unsupported");
    }
    if !are_valid_volume_labels(&request.labels) {
        return field_error("labels", "invalid");
    }
    match all_members(&context, std::slice::from_ref(&request.node_id)).await {
        Some(true) => {}
        Some(false) => return field_error("nodeId", "invalid"),
        None => return admission_error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable"),
    }
    let scope = format!("/volumes/{}", request.node_id);
    let payload = CreateVolumePayload {
        node_id: request.node_id,
        name: request.name,
        driver: request.driver,
        labels: request.labels,
    };
    admit(
        &context,
        Admission {
            principal,
            key,
            request_id,
            scope: &scope,
            kind: OperationKind::VolumeCreate,
            payload: serde_json::to_value(payload).unwrap_or_default(),
        },
    )
    .await
}

#[utoipa::path(
    delete,
    path = "/api/v1/volumes/{nodeId}/{name}",
    operation_id = "remove_volume",
    params(
        ("nodeId" = String, Path, description = "Member node that holds the volume"),
        ("name" = String, Path, description = "Volume name"),
        ("Idempotency-Key" = String, Header, description = "16–64 printable ASCII characters, one per user gesture")
    ),
    responses(
        (status = 202, description = "Admitted as one `volume.remove` operation; a mounted volume fails it with `volume_in_use` and the containers' names", body = super::admission::AdmittedResponse),
        (status = 400, description = "`idempotency_key_required`, or `invalid_request` naming `nodeId` or `name`", body = super::admission::FieldErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 409, description = "`idempotency_conflict`", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "Persistence unavailable or controller draining")
    ),
    security(("access_session" = []))
)]
pub(crate) async fn remove(
    State(context): State<AppContext>,
    principal: PrincipalContext,
    Path((node_id, name)): Path<(String, String)>,
    key: IdempotencyKey,
    AdmittingRequestId(request_id): AdmittingRequestId,
) -> Response {
    if !is_valid_volume_name(&name) {
        return field_error("name", "invalid");
    }
    match all_members(&context, std::slice::from_ref(&node_id)).await {
        Some(true) => {}
        Some(false) => return field_error("nodeId", "invalid"),
        None => return admission_error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable"),
    }
    let scope = format!("/volumes/{node_id}/{name}");
    let payload = RemoveVolumePayload { node_id, name };
    admit(
        &context,
        Admission {
            principal,
            key,
            request_id,
            scope: &scope,
            kind: OperationKind::VolumeRemove,
            payload: serde_json::to_value(payload).unwrap_or_default(),
        },
    )
    .await
}

pub fn routes() -> Routes {
    Routes::new()
        .get("/volumes", index)
        .add("/volumes", post(create))
        .add("/volumes/{nodeId}/{name}", delete(remove))
}
