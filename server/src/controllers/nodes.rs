//! Node membership and per-node samples.

use axum::{
    Json,
    extract::{Path, RawQuery, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, SecondsFormat, Utc};
use loco_rs::{app::AppContext, controller::Routes};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    auth::domain::PrincipalContext,
    metrics::{
        actions::node_samples,
        domain::{SamplePoint, SampleRange, SampleScope},
    },
    nodes::{
        actions::list_nodes,
        domain::{LatestSample, NodeStatus, NodeView},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatusResponse {
    Online,
    Degraded,
    Offline,
}

impl From<NodeStatus> for NodeStatusResponse {
    fn from(status: NodeStatus) -> Self {
        match status {
            NodeStatus::Online => Self::Online,
            NodeStatus::Degraded => Self::Degraded,
            NodeStatus::Offline => Self::Offline,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodeSampleResponse {
    pub sampled_at: String,
    /// 0–100 of the node.
    pub cpu_percent: f32,
    pub memory_used_bytes: i64,
    #[schema(required = true)]
    pub disk_used_bytes: Option<i64>,
    pub containers_running: i32,
    pub containers_paused: i32,
    pub containers_exited: i32,
    #[schema(required = true)]
    pub net_rx_bytes_per_sec: Option<f64>,
    #[schema(required = true)]
    pub net_tx_bytes_per_sec: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodeResponse {
    /// Node id from the agent's certificate identity.
    pub id: String,
    pub hostname: String,
    /// Derived from the last check-in: `online` within 15 s, `degraded` within 60 s.
    pub status: NodeStatusResponse,
    /// Swarm fields are null until Swarm inventory fills them.
    #[schema(required = true)]
    pub swarm_node_id: Option<String>,
    #[schema(required = true)]
    pub role: Option<String>,
    #[schema(required = true)]
    pub availability: Option<String>,
    pub agent_version: String,
    pub docker_version: String,
    pub os: String,
    pub arch: String,
    pub cpu_cores: i32,
    pub memory_total_bytes: i64,
    pub agent_started_at: String,
    pub first_seen_at: String,
    pub last_check_in_at: String,
    #[schema(required = true)]
    pub latest_sample: Option<NodeSampleResponse>,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodeListResponse {
    pub items: Vec<NodeResponse>,
    pub snapshot_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SamplePointResponse {
    /// Bucket start.
    pub at: String,
    /// Null when no sample landed in the bucket, never 0.
    #[schema(required = true)]
    pub cpu_percent: Option<f64>,
    #[schema(required = true)]
    pub memory_used_bytes: Option<f64>,
    #[schema(required = true)]
    pub disk_used_bytes: Option<f64>,
    #[schema(required = true)]
    pub containers_running: Option<f64>,
    #[schema(required = true)]
    pub net_rx_bytes_per_sec: Option<f64>,
    #[schema(required = true)]
    pub net_tx_bytes_per_sec: Option<f64>,
    /// Nodes that contributed a sample to the bucket.
    pub nodes: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SampleSeriesResponse {
    pub window: String,
    pub bucket: String,
    pub points: Vec<SamplePointResponse>,
    pub snapshot_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NodeErrorResponse {
    pub error: String,
}

pub(crate) fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub(crate) fn json_no_store(status: StatusCode, value: impl Serialize) -> Response {
    (status, [(header::CACHE_CONTROL, "no-store")], Json(value)).into_response()
}

pub(crate) fn error(status: StatusCode, code: &str) -> Response {
    json_no_store(
        status,
        NodeErrorResponse {
            error: code.to_owned(),
        },
    )
}

fn sample_response(sample: LatestSample) -> NodeSampleResponse {
    NodeSampleResponse {
        sampled_at: timestamp(sample.sampled_at),
        cpu_percent: sample.cpu_percent,
        memory_used_bytes: sample.memory_used_bytes,
        disk_used_bytes: sample.disk_used_bytes,
        containers_running: sample.containers_running,
        containers_paused: sample.containers_paused,
        containers_exited: sample.containers_exited,
        net_rx_bytes_per_sec: sample.net_rx_bytes_per_sec,
        net_tx_bytes_per_sec: sample.net_tx_bytes_per_sec,
    }
}

fn node_response(node: NodeView) -> NodeResponse {
    NodeResponse {
        id: node.id,
        hostname: node.hostname,
        status: node.status.into(),
        swarm_node_id: node.swarm_node_id,
        role: None,
        availability: None,
        agent_version: node.agent_version,
        docker_version: node.docker_version,
        os: node.os,
        arch: node.arch,
        cpu_cores: node.cpu_cores,
        memory_total_bytes: node.memory_total_bytes,
        agent_started_at: timestamp(node.agent_started_at),
        first_seen_at: timestamp(node.first_seen_at),
        last_check_in_at: timestamp(node.last_check_in_at),
        latest_sample: node.latest_sample.map(sample_response),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/nodes",
    operation_id = "list_nodes",
    responses(
        (status = 200, description = "Every node that has checked in, with derived status and latest sample", body = NodeListResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 503, description = "Persistence unavailable or controller draining", body = NodeErrorResponse)
    ),
    security(("access_session" = []))
)]
pub(crate) async fn index(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
) -> Response {
    match list_nodes::execute(&context.db).await {
        Ok((nodes, now)) => json_no_store(
            StatusCode::OK,
            NodeListResponse {
                items: nodes.into_iter().map(node_response).collect(),
                snapshot_at: timestamp(now),
            },
        ),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable"),
    }
}

/// Strict `window` and `bucket` query: both required, nothing else allowed.
pub(crate) fn parse_range(raw: Option<&str>) -> Result<(SampleRange, String, String), ()> {
    let mut window = None;
    let mut bucket = None;
    for (key, value) in url::form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
        let slot = match key.as_ref() {
            "window" => &mut window,
            "bucket" => &mut bucket,
            _ => return Err(()),
        };
        if slot.replace(value.into_owned()).is_some() {
            return Err(());
        }
    }
    let (Some(window), Some(bucket)) = (window, bucket) else {
        return Err(());
    };
    let range = SampleRange::parse(&window, &bucket).map_err(|_| ())?;
    Ok((range, window, bucket))
}

pub(crate) async fn sample_series(
    context: &AppContext,
    scope: SampleScope,
    query: Option<&str>,
) -> Response {
    let Ok((range, window, bucket)) = parse_range(query) else {
        return error(StatusCode::BAD_REQUEST, "invalid_range");
    };
    match node_samples::execute(&context.db, &scope, range).await {
        Ok((points, now)) => json_no_store(
            StatusCode::OK,
            SampleSeriesResponse {
                window,
                bucket,
                points: points.into_iter().map(point_response).collect(),
                snapshot_at: timestamp(now),
            },
        ),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable"),
    }
}

fn point_response(point: SamplePoint) -> SamplePointResponse {
    SamplePointResponse {
        at: timestamp(point.at),
        cpu_percent: point.cpu_percent,
        memory_used_bytes: point.memory_used_bytes,
        disk_used_bytes: point.disk_used_bytes,
        containers_running: point.containers_running,
        net_rx_bytes_per_sec: point.net_rx_bytes_per_sec,
        net_tx_bytes_per_sec: point.net_tx_bytes_per_sec,
        nodes: point.nodes,
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/nodes/{nodeId}/samples",
    operation_id = "node_samples",
    params(
        ("nodeId" = String, Path, description = "Node id"),
        ("window" = String, Query, description = "One of 5m, 15m, 1h, 24h, 7d"),
        ("bucket" = String, Query, description = "Bucket width such as 5s, 60s, 15m; at most 200 points")
    ),
    responses(
        (status = 200, description = "Averages per bucket; buckets without samples are null", body = SampleSeriesResponse),
        (status = 400, description = "Invalid window or bucket", body = NodeErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 404, description = "Unknown node", body = NodeErrorResponse),
        (status = 503, description = "Persistence unavailable or controller draining", body = NodeErrorResponse)
    ),
    security(("access_session" = []))
)]
pub(crate) async fn samples(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
    Path(node_id): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    use sea_orm::EntityTrait;

    match crate::nodes::entities::node::Entity::find_by_id(node_id.clone())
        .one(&context.db)
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => return error(StatusCode::NOT_FOUND, "not_found"),
        Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable"),
    }
    sample_series(&context, SampleScope::Node(node_id), query.as_deref()).await
}

#[utoipa::path(
    get,
    path = "/api/v1/cluster/samples",
    operation_id = "cluster_samples",
    params(
        ("window" = String, Query, description = "One of 5m, 15m, 1h, 24h, 7d"),
        ("bucket" = String, Query, description = "Bucket width such as 5s, 60s, 15m; at most 200 points")
    ),
    responses(
        (status = 200, description = "Per bucket: mean CPU over contributing nodes, summed figures, and the contributor count", body = SampleSeriesResponse),
        (status = 400, description = "Invalid window or bucket", body = NodeErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 503, description = "Persistence unavailable or controller draining", body = NodeErrorResponse)
    ),
    security(("access_session" = []))
)]
pub(crate) async fn cluster_samples(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
    RawQuery(query): RawQuery,
) -> Response {
    sample_series(&context, SampleScope::Cluster, query.as_deref()).await
}

pub fn routes() -> Routes {
    Routes::new()
        .get("/nodes", index)
        .get("/nodes/{nodeId}/samples", samples)
        .get("/cluster/samples", cluster_samples)
}

#[cfg(test)]
mod tests {
    use super::parse_range;

    #[test]
    fn ranges_require_exactly_window_and_bucket() {
        let (range, window, bucket) = parse_range(Some("window=24h&bucket=15m")).unwrap();
        assert_eq!(range.points(), 96);
        assert_eq!((window.as_str(), bucket.as_str()), ("24h", "15m"));

        for invalid in [
            None,
            Some(""),
            Some("window=1h"),
            Some("bucket=60s"),
            Some("window=1h&bucket=60s&extra=1"),
            Some("window=1h&window=1h&bucket=60s"),
            Some("window=2h&bucket=60s"),
            Some("window=24h&bucket=1s"),
        ] {
            assert!(parse_range(invalid).is_err(), "{invalid:?}");
        }
    }
}
