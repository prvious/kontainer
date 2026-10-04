//! Node membership and per-node samples.

use axum::{
    Json,
    extract::{Path, RawQuery, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, SecondsFormat, Utc};
use kontainer_engine::{SwarmAvailability, SwarmNodeState};
use loco_rs::{app::AppContext, controller::Routes};
use serde::Serialize;
use utoipa::ToSchema;

use super::services::shutdown_token;
use crate::{
    auth::domain::PrincipalContext,
    metrics::{
        actions::node_samples,
        domain::{SamplePoint, SampleRange, SampleScope},
    },
    nodes::{
        actions::{cluster_summary, list_cluster_nodes},
        domain::{
            AgentStatus, ClusterNode, ClusterSummary, LatestSample, NodeHealth, NodeRole,
            QuorumState,
        },
    },
    swarm::engine::SharedManagerEngine,
};

/// The agent side: derived from the last check-in, or `no-agent` for a Swarm member that never
/// checked in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AgentStatusResponse {
    Online,
    Degraded,
    Offline,
    NoAgent,
}

impl From<AgentStatus> for AgentStatusResponse {
    fn from(status: AgentStatus) -> Self {
        match status {
            AgentStatus::Online => Self::Online,
            AgentStatus::Degraded => Self::Degraded,
            AgentStatus::Offline => Self::Offline,
            AgentStatus::NoAgent => Self::NoAgent,
        }
    }
}

/// `active` when the agent is online and Swarm says ready; `down` when either says down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeHealthResponse {
    Active,
    Degraded,
    Down,
}

impl From<NodeHealth> for NodeHealthResponse {
    fn from(health: NodeHealth) -> Self {
        match health {
            NodeHealth::Active => Self::Active,
            NodeHealth::Degraded => Self::Degraded,
            NodeHealth::Down => Self::Down,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeRoleResponse {
    Manager,
    Worker,
    Unknown,
}

impl From<NodeRole> for NodeRoleResponse {
    fn from(role: NodeRole) -> Self {
        match role {
            NodeRole::Manager => Self::Manager,
            NodeRole::Worker => Self::Worker,
            NodeRole::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeAvailabilityResponse {
    Active,
    Pause,
    Drain,
}

impl From<SwarmAvailability> for NodeAvailabilityResponse {
    fn from(availability: SwarmAvailability) -> Self {
        match availability {
            SwarmAvailability::Active => Self::Active,
            SwarmAvailability::Pause => Self::Pause,
            SwarmAvailability::Drain => Self::Drain,
        }
    }
}

/// What the manager last heard from the node; `unknown` when it did not answer or does not
/// know the node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SwarmNodeStatusResponse {
    Ready,
    Down,
    Unknown,
}

impl From<SwarmNodeState> for SwarmNodeStatusResponse {
    fn from(state: SwarmNodeState) -> Self {
        match state {
            SwarmNodeState::Ready => Self::Ready,
            SwarmNodeState::Down => Self::Down,
            SwarmNodeState::Unknown => Self::Unknown,
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

/// One node: a membership row joined to the manager's Swarm member.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodeResponse {
    /// The agent's certificate identity; the Swarm node id for a member without an agent.
    pub id: String,
    pub hostname: String,
    pub agent_status: AgentStatusResponse,
    pub health: NodeHealthResponse,
    pub role: NodeRoleResponse,
    pub is_leader: bool,
    #[schema(required = true)]
    pub availability: Option<NodeAvailabilityResponse>,
    pub swarm_status: SwarmNodeStatusResponse,
    #[schema(required = true)]
    pub swarm_node_id: Option<String>,
    /// The address the manager sees the node at.
    #[schema(required = true)]
    pub address: Option<String>,
    /// Agent fields are null for a member without an agent.
    #[schema(required = true)]
    pub agent_version: Option<String>,
    pub docker_version: String,
    pub os: String,
    pub arch: String,
    pub cpu_cores: u32,
    pub memory_total_bytes: u64,
    #[schema(required = true)]
    pub agent_started_at: Option<String>,
    #[schema(required = true)]
    pub first_seen_at: Option<String>,
    #[schema(required = true)]
    pub last_check_in_at: Option<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuorumStateResponse {
    Healthy,
    Degraded,
}

/// Counts by health, manager reachability, quorum, and load over online agents.
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClusterSummaryResponse {
    pub nodes_total: u32,
    pub nodes_active: u32,
    pub nodes_degraded: u32,
    pub nodes_down: u32,
    /// Null when the manager did not answer: unknown, not zero.
    #[schema(required = true)]
    pub managers_total: Option<u32>,
    /// Null when the manager did not answer.
    #[schema(required = true)]
    pub managers_reachable: Option<u32>,
    /// `healthy` when the manager answers and every manager is ready and reachable.
    pub quorum: QuorumStateResponse,
    /// Mean over online nodes with a sample; null when there are none.
    #[schema(required = true)]
    pub cpu_percent: Option<f64>,
    /// Summed over the same nodes.
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub containers_running: u64,
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

fn node_response(node: ClusterNode) -> NodeResponse {
    let agent_status = node.agent_status().into();
    let health = node.health().into();
    let role = node.role().into();
    let swarm_status = node.swarm_state().into();
    let ClusterNode { agent, swarm } = node;
    let is_leader = swarm.as_ref().is_some_and(|swarm| swarm.is_leader);
    let availability = swarm
        .as_ref()
        .and_then(|swarm| swarm.availability)
        .map(Into::into);
    let address = swarm.as_ref().and_then(|swarm| swarm.address.clone());
    match (agent, swarm) {
        (Some(agent), swarm) => NodeResponse {
            id: agent.id,
            hostname: agent.hostname,
            agent_status,
            health,
            role,
            is_leader,
            availability,
            swarm_status,
            swarm_node_id: agent.swarm_node_id,
            address,
            agent_version: Some(agent.agent_version),
            docker_version: agent.docker_version,
            os: agent.os,
            arch: agent.arch,
            cpu_cores: u32::try_from(agent.cpu_cores)
                .ok()
                .or(swarm.as_ref().and_then(|swarm| swarm.cpu_cores))
                .unwrap_or(0),
            memory_total_bytes: u64::try_from(agent.memory_total_bytes).unwrap_or(0),
            agent_started_at: Some(timestamp(agent.agent_started_at)),
            first_seen_at: Some(timestamp(agent.first_seen_at)),
            last_check_in_at: Some(timestamp(agent.last_check_in_at)),
            latest_sample: agent.latest_sample.map(sample_response),
        },
        (None, Some(swarm)) => NodeResponse {
            swarm_node_id: Some(swarm.id.clone()),
            id: swarm.id,
            hostname: swarm.hostname,
            agent_status,
            health,
            role,
            is_leader,
            availability,
            swarm_status,
            address,
            agent_version: None,
            docker_version: swarm.docker_version.unwrap_or_default(),
            os: swarm.os.unwrap_or_default(),
            arch: swarm.architecture.unwrap_or_default(),
            cpu_cores: swarm.cpu_cores.unwrap_or(0),
            memory_total_bytes: swarm.memory_total_bytes.unwrap_or(0),
            agent_started_at: None,
            first_seen_at: None,
            last_check_in_at: None,
            latest_sample: None,
        },
        (None, None) => unreachable!("a cluster node has an agent, a Swarm member, or both"),
    }
}

fn summary_response(summary: ClusterSummary, now: DateTime<Utc>) -> ClusterSummaryResponse {
    ClusterSummaryResponse {
        nodes_total: summary.nodes_total,
        nodes_active: summary.nodes_active,
        nodes_degraded: summary.nodes_degraded,
        nodes_down: summary.nodes_down,
        managers_total: summary.managers_total,
        managers_reachable: summary.managers_reachable,
        quorum: match summary.quorum {
            QuorumState::Healthy => QuorumStateResponse::Healthy,
            QuorumState::Degraded => QuorumStateResponse::Degraded,
        },
        cpu_percent: summary.cpu_percent,
        memory_used_bytes: summary.memory_used_bytes,
        memory_total_bytes: summary.memory_total_bytes,
        containers_running: summary.containers_running,
        snapshot_at: timestamp(now),
    }
}

fn optional_manager(context: &AppContext) -> Option<SharedManagerEngine> {
    context.shared_store.get::<SharedManagerEngine>()
}

#[utoipa::path(
    get,
    path = "/api/v1/nodes",
    operation_id = "list_nodes",
    responses(
        (status = 200, description = "Every node that checked in or that Swarm lists, joined by Swarm node id, with derived health and latest sample. Swarm fields are unknown when the manager does not answer.", body = NodeListResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 503, description = "Persistence unavailable or controller draining", body = NodeErrorResponse)
    ),
    security(("access_session" = []))
)]
pub(crate) async fn index(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
) -> Response {
    let manager = optional_manager(&context);
    let cancellation = shutdown_token(&context);
    match list_cluster_nodes::execute(
        &context.db,
        manager.as_ref().map(|manager| manager.0.as_ref()),
        &cancellation,
    )
    .await
    {
        Ok(cluster) => json_no_store(
            StatusCode::OK,
            NodeListResponse {
                items: cluster.nodes.into_iter().map(node_response).collect(),
                snapshot_at: timestamp(cluster.snapshot_at),
            },
        ),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable"),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/cluster/summary",
    operation_id = "cluster_summary",
    responses(
        (status = 200, description = "Node counts by health, manager reachability and quorum, and load summed over online nodes", body = ClusterSummaryResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 503, description = "Persistence unavailable or controller draining", body = NodeErrorResponse)
    ),
    security(("access_session" = []))
)]
pub(crate) async fn cluster_summary(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
) -> Response {
    let manager = optional_manager(&context);
    let cancellation = shutdown_token(&context);
    match cluster_summary::execute(
        &context.db,
        manager.as_ref().map(|manager| manager.0.as_ref()),
        &cancellation,
    )
    .await
    {
        Ok((summary, now)) => json_no_store(StatusCode::OK, summary_response(summary, now)),
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
        .get("/cluster/summary", cluster_summary)
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
