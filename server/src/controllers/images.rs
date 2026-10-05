#![forbid(unsafe_code)]

//! Images across the cluster: the merged list, removal, and pulls.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Response,
};
use chrono::{DateTime, SecondsFormat, Utc};
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
    nodes::json_no_store,
};
use crate::{
    auth::domain::PrincipalContext,
    images::{
        actions::list_images,
        domain::{
            ImageRow, NodeSelection, PullImagePayload, ReferenceError, RemoveImagePayload,
            normalize_reference,
        },
    },
    nodes::{
        agents::SharedAgentDirectory,
        entities::node,
        fan_out::{FanOutResponse, NodeOutcomeResponse},
    },
    operations::domain::OperationKind,
};

pub(super) const MAX_TARGET_NODES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageResponse {
    /// `sha256:<64 hex>`.
    pub image_id: String,
    pub repo_tags: Vec<String>,
    pub repo_digests: Vec<String>,
    /// Counted once, however many nodes have it.
    pub size_bytes: u64,
    pub created_at: String,
    /// Containers using it, summed across nodes.
    pub used_by: u32,
    pub node_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageListResponse {
    pub items: Vec<ImageResponse>,
    pub nodes: Vec<NodeOutcomeResponse>,
    pub snapshot_at: String,
}

fn timestamp(seconds: i64) -> String {
    DateTime::<Utc>::from_timestamp(seconds, 0)
        .unwrap_or(DateTime::UNIX_EPOCH)
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn image_response(row: ImageRow) -> ImageResponse {
    ImageResponse {
        created_at: timestamp(row.created_at_seconds),
        image_id: row.image_id,
        repo_tags: row.repo_tags,
        repo_digests: row.repo_digests,
        size_bytes: row.size_bytes,
        used_by: row.used_by,
        node_ids: row.node_ids,
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/images",
    operation_id = "list_images",
    responses(
        (status = 200, description = "Images merged by id across answering nodes, one outcome per member", body = ImageListResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 503, description = "Agent RPC not configured, persistence unavailable, or controller draining", body = super::admission::AdmissionErrorResponse)
    )
)]
pub(crate) async fn index(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
) -> Response {
    let Some(directory) = context.shared_store.get::<SharedAgentDirectory>() else {
        return admission_error(StatusCode::SERVICE_UNAVAILABLE, "agent_unavailable");
    };
    let shutdown = context
        .shared_store
        .get::<crate::server::ForceCancellation>()
        .map_or_else(CancellationToken::new, |force| force.0);
    match list_images::execute(&context.db, directory.0.as_ref(), &shutdown).await {
        Ok(list) => {
            let outcomes = FanOutResponse::<ImageResponse>::from_outcomes(
                list.outcomes,
                list.snapshot_at,
                |_, ()| Vec::new(),
            );
            json_no_store(
                StatusCode::OK,
                ImageListResponse {
                    items: list.items.into_iter().map(image_response).collect(),
                    nodes: outcomes.nodes,
                    snapshot_at: outcomes.snapshot_at,
                },
            )
        }
        Err(_) => admission_error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable"),
    }
}

fn valid_image_id(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

/// Every listed node must be a member; `None` when membership could not be read.
pub(super) async fn all_members(context: &AppContext, node_ids: &[String]) -> Option<bool> {
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
    let distinct: std::collections::BTreeSet<_> = node_ids.iter().collect();
    if distinct.len() != node_ids.len()
        || node_ids.is_empty()
        || node_ids.len() > MAX_TARGET_NODES
        || !node_ids
            .iter()
            .all(|node_id| kontainer_rpc_auth::valid_identity_segment(node_id))
    {
        return Some(false);
    }
    let found = node::Entity::find()
        .filter(node::Column::Id.is_in(node_ids.to_vec()))
        .count(&context.db)
        .await
        .ok()?;
    Some(usize::try_from(found).is_ok_and(|found| found == node_ids.len()))
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveImageRequest {
    /// Node ids, or `"all"` for every member that has it.
    #[schema(value_type = Object)]
    pub node_ids: NodeSelection,
    /// Remove even while containers use it.
    #[serde(default)]
    pub force: bool,
}

#[utoipa::path(
    delete,
    path = "/api/v1/images/{imageId}",
    operation_id = "remove_image",
    params(
        ("imageId" = String, Path, description = "`sha256:<64 hex>`"),
        ("Idempotency-Key" = String, Header, description = "16–64 printable ASCII characters, one per user gesture")
    ),
    request_body = RemoveImageRequest,
    responses(
        (status = 202, description = "Admitted as one `image.remove` operation", body = super::admission::AdmittedResponse),
        (status = 400, description = "`idempotency_key_required` or `invalid_request` with the field", body = super::admission::FieldErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 409, description = "`idempotency_conflict`", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "Persistence unavailable or controller draining")
    )
)]
pub(crate) async fn remove(
    State(context): State<AppContext>,
    principal: PrincipalContext,
    Path(image_id): Path<String>,
    key: IdempotencyKey,
    AdmittingRequestId(request_id): AdmittingRequestId,
    body: axum::body::Bytes,
) -> Response {
    if !valid_image_id(&image_id) {
        return field_error("imageId", "invalid");
    }
    let Ok(request) = serde_json::from_slice::<RemoveImageRequest>(&body) else {
        return field_error("nodeIds", "invalid");
    };
    if let NodeSelection::Nodes(node_ids) = &request.node_ids {
        match all_members(&context, node_ids).await {
            Some(true) => {}
            Some(false) => return field_error("nodeIds", "invalid"),
            None => {
                return admission_error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable");
            }
        }
    }
    let scope = format!("/images/{image_id}");
    let payload = RemoveImagePayload {
        image_id,
        node_ids: request.node_ids,
        force: request.force,
    };
    admit(
        &context,
        Admission {
            principal,
            key,
            request_id,
            scope: &scope,
            kind: OperationKind::ImageRemove,
            payload: serde_json::to_value(payload).unwrap_or_default(),
        },
    )
    .await
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PullImageRequest {
    /// `[host[:port]/]name[:tag|@digest]`; `:latest` is added when neither is given.
    pub reference: String,
    /// 1–64 member nodes.
    pub node_ids: Vec<String>,
}

fn reference_problem(error: ReferenceError) -> &'static str {
    match error {
        ReferenceError::Length => "empty_or_too_long",
        ReferenceError::Domain => "invalid_registry",
        ReferenceError::Name => "invalid_name",
        ReferenceError::Tag => "invalid_tag",
        ReferenceError::Digest => "invalid_digest",
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/images/pulls",
    operation_id = "pull_image",
    params(("Idempotency-Key" = String, Header, description = "16–64 printable ASCII characters, one per user gesture")),
    request_body = PullImageRequest,
    responses(
        (status = 202, description = "Admitted as one `image.pull` operation; poll it for per-node progress", body = super::admission::AdmittedResponse),
        (status = 400, description = "`idempotency_key_required`, or `invalid_request` naming `reference` or `nodeIds`", body = super::admission::FieldErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 409, description = "`idempotency_conflict`", body = super::admission::AdmissionErrorResponse),
        (status = 503, description = "Persistence unavailable or controller draining")
    )
)]
pub(crate) async fn pull(
    State(context): State<AppContext>,
    principal: PrincipalContext,
    key: IdempotencyKey,
    AdmittingRequestId(request_id): AdmittingRequestId,
    body: axum::body::Bytes,
) -> Response {
    let Ok(request) = serde_json::from_slice::<PullImageRequest>(&body) else {
        return field_error("reference", "invalid");
    };
    let reference = match normalize_reference(&request.reference) {
        Ok(reference) => reference,
        Err(error) => return field_error("reference", reference_problem(error)),
    };
    match all_members(&context, &request.node_ids).await {
        Some(true) => {}
        Some(false) => return field_error("nodeIds", "invalid"),
        None => return admission_error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable"),
    }
    let payload = PullImagePayload {
        reference,
        node_ids: request.node_ids,
    };
    admit(
        &context,
        Admission {
            principal,
            key,
            request_id,
            scope: "/images/pulls",
            kind: OperationKind::ImagePull,
            payload: serde_json::to_value(payload).unwrap_or_default(),
        },
    )
    .await
}

pub fn routes() -> Routes {
    Routes::new()
        .get("/images", index)
        .add("/images/pulls", post(pull))
        .add("/images/{imageId}", delete(remove))
}
