#![forbid(unsafe_code)]

//! Bounded public container collection for the single configured node.
//!
//! The `{nodeId}` path segment can never steer the RPC: it is compared
//! exactly with the statically configured node ID before any outbound call.
//! Pagination is validated locally against the #82 bounds; malformed values
//! return `400 invalid_pagination` without touching the agent. Successful
//! pages preserve the agent's deterministic order and opaque cursor.

use axum::{
    Json,
    extract::{Path, RawQuery, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::SecondsFormat;
use loco_rs::{app::AppContext, controller::Routes};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    auth::domain::PrincipalContext,
    rpc::{
        AgentClientError, AgentContainerState, SharedAgentClient, client::is_valid_container_id,
    },
};

pub(crate) const DEFAULT_CONTAINER_LIMIT: u32 = 50;
pub(crate) const MAX_CONTAINER_LIMIT: u32 = 100;
pub(crate) const MAX_CONTAINER_CURSOR_LEN: usize = 67;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContainerItemResponse {
    /// Full lowercase-hex daemon container ID.
    pub container_id: String,
    /// Canonical display name without Docker linkage.
    pub display_name: String,
    /// Image reference as reported by the daemon.
    pub image_reference: String,
    /// Typed lifecycle state.
    pub state: ContainerStateResponse,
    /// Bounded daemon status text.
    pub status: String,
    /// RFC 3339 UTC creation time with millisecond precision.
    pub created_at: String,
    /// Serving node ID from agent configuration, never request input.
    pub node_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContainerPageResponse {
    /// One bounded page preserving the agent's deterministic newest-first
    /// order. Live collections are best-effort: containers created or removed
    /// between calls can shift pages. A cursor whose target was removed is
    /// rejected as invalid pagination; restart traversal from the first page.
    #[schema(max_items = 100)]
    pub items: Vec<ContainerItemResponse>,
    /// Opaque cursor for the next page, or null when this page is last.
    /// Pass back unchanged; clients never decode or synthesize it.
    #[schema(required = true)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ContainerErrorResponse {
    pub error: String,
}

fn json_no_store(status: StatusCode, value: impl Serialize) -> Response {
    (
        status,
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::PRAGMA, "no-cache"),
        ],
        Json(value),
    )
        .into_response()
}

fn error_body(status: StatusCode, code: &str) -> Response {
    json_no_store(
        status,
        ContainerErrorResponse {
            error: code.to_owned(),
        },
    )
}

fn invalid_pagination() -> Response {
    error_body(StatusCode::BAD_REQUEST, "invalid_pagination")
}

fn not_found() -> Response {
    error_body(StatusCode::NOT_FOUND, "not_found")
}

fn agent_unavailable() -> Response {
    error_body(StatusCode::SERVICE_UNAVAILABLE, "agent_unavailable")
}

fn agent_timeout() -> Response {
    error_body(StatusCode::GATEWAY_TIMEOUT, "agent_timeout")
}

fn map_agent_error(error: AgentClientError) -> Response {
    match error {
        AgentClientError::InvalidPagination => invalid_pagination(),
        AgentClientError::TimedOut => agent_timeout(),
        AgentClientError::Unavailable
        | AgentClientError::IdentityMismatch
        | AgentClientError::InvalidResponse => agent_unavailable(),
        // Caller disconnect drops this response; shutdown drain reports the
        // dependency as unavailable without leaking which side cancelled.
        AgentClientError::Cancelled => agent_unavailable(),
    }
}

/// Strict pagination decoding: unknown or duplicated keys, non-numeric or
/// out-of-range limits, and malformed or oversized cursors are all
/// `invalid_pagination`. Missing `limit` selects the documented default.
pub(crate) fn parse_pagination(raw: Option<&str>) -> Result<(u32, String), ()> {
    let Some(query) = raw.filter(|query| !query.is_empty()) else {
        return Ok((DEFAULT_CONTAINER_LIMIT, String::new()));
    };
    let mut limit_seen = 0;
    let mut cursor_seen = 0;
    let mut limit_raw: Option<String> = None;
    let mut cursor_raw: Option<String> = None;
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        match key.as_ref() {
            "limit" => {
                limit_seen += 1;
                limit_raw = Some(value.into_owned());
            }
            "cursor" => {
                cursor_seen += 1;
                cursor_raw = Some(value.into_owned());
            }
            _ => return Err(()),
        }
    }
    if limit_seen > 1 || cursor_seen > 1 {
        return Err(());
    }
    let limit = match limit_raw {
        None => DEFAULT_CONTAINER_LIMIT,
        Some(raw) => parse_limit(&raw)?,
    };
    let cursor = match cursor_raw {
        None => String::new(),
        Some(raw) => {
            if raw.len() > MAX_CONTAINER_CURSOR_LEN || !is_valid_cursor(&raw) {
                return Err(());
            }
            raw
        }
    };
    Ok((limit, cursor))
}

fn parse_limit(raw: &str) -> Result<u32, ()> {
    if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(());
    }
    // Digits-only, so parsing as u64 cannot fail on sign or whitespace; the
    // range check below enforces the 1..=100 bound.
    let value: u64 = raw.parse().map_err(|_| ())?;
    if value < 1 || u64::from(MAX_CONTAINER_LIMIT) < value {
        return Err(());
    }
    Ok(value as u32)
}

pub(crate) fn is_valid_cursor(token: &str) -> bool {
    if token.is_empty() {
        return true;
    }
    if token.len() > MAX_CONTAINER_CURSOR_LEN {
        return false;
    }
    token.strip_prefix("v1:").is_some_and(is_valid_container_id)
}

#[utoipa::path(
    get,
    path = "/api/v1/nodes/{nodeId}/containers",
    params(
        ("nodeId" = String, Path, description = "Configured node ID; only the statically configured node is served"),
        ("limit" = Option<u32>, Query, description = "Page size; defaults to 50, range 1-100", minimum = 1, maximum = 100),
        ("cursor" = Option<String>, Query, description = "Opaque cursor from a previous page; empty means the first page", max_length = 67)
    ),
    responses(
        (status = 200, description = "One bounded page preserving agent order; live collections are best-effort", body = ContainerPageResponse),
        (status = 400, description = "Invalid pagination; no agent call was made", body = ContainerErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 404, description = "Unknown node; only the configured node exists", body = ContainerErrorResponse),
        (status = 503, description = "Agent unavailable or response invalid; no transport or daemon detail", body = ContainerErrorResponse),
        (status = 504, description = "Agent deadline expired", body = ContainerErrorResponse)
    )
)]
pub(crate) async fn index(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
    Path(node_id): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    let Some(client) = context.shared_store.get::<SharedAgentClient>() else {
        return agent_unavailable();
    };
    // The URL node selects nothing: it must equal the static configuration
    // exactly, otherwise no RPC leaves the controller.
    if node_id != client.0.node_id() {
        return not_found();
    }
    let Ok((limit, cursor)) = parse_pagination(query.as_deref()) else {
        return invalid_pagination();
    };
    let Some(shutdown) = context
        .shared_store
        .get::<crate::server::ForceCancellation>()
    else {
        return agent_unavailable();
    };

    // Directly await the RPC: dropping the HTTP request drops this future,
    // so cancellation joins the in-flight call without detached work.
    let page = match client.0.list_containers(limit, cursor, &shutdown.0).await {
        Ok(page) => page,
        Err(error) => return map_agent_error(error),
    };

    let mut items = Vec::with_capacity(page.containers.len());
    for container in page.containers {
        // Defense in depth with the client boundary: negative daemon times
        // never become 1969 pages even if a future client lets one through.
        if container.created_at_seconds < 0 {
            return agent_unavailable();
        }
        let Some(created_at) = chrono::DateTime::from_timestamp(container.created_at_seconds, 0)
        else {
            return agent_unavailable();
        };
        items.push(ContainerItemResponse {
            container_id: container.container_id,
            display_name: container.display_name,
            image_reference: container.image_reference,
            state: container.state.into(),
            status: container.status,
            created_at: created_at.to_rfc3339_opts(SecondsFormat::Millis, true),
            node_id: container.node_id,
        });
    }

    json_no_store(
        StatusCode::OK,
        ContainerPageResponse {
            items,
            next_cursor: page.next_page_token,
        },
    )
}

pub fn routes() -> Routes {
    Routes::new().get("/nodes/{nodeId}/containers", index)
}

#[cfg(test)]
mod tests {
    use super::{
        AgentClientError, AgentContainerState, ContainerStateResponse, MAX_CONTAINER_CURSOR_LEN,
        MAX_CONTAINER_LIMIT, is_valid_cursor, map_agent_error, parse_limit, parse_pagination,
    };
    use crate::rpc::{AgentContainerPage, AgentContainerSummary};
    use axum::http::StatusCode;

    fn summary(id: &str) -> AgentContainerSummary {
        AgentContainerSummary {
            container_id: id.to_owned(),
            display_name: "web".to_owned(),
            image_reference: "nginx:latest".to_owned(),
            state: AgentContainerState::Running,
            status: "Up 3s".to_owned(),
            created_at_seconds: 1_700_000_000,
            node_id: "node-1".to_owned(),
        }
    }

    #[test]
    fn pagination_defaults_and_rejects_without_agent_detail() {
        assert_eq!(
            parse_pagination(None).unwrap(),
            (50, String::new()),
            "missing query selects the documented default"
        );
        assert_eq!(parse_pagination(Some("")).unwrap(), (50, String::new()));
        assert_eq!(
            parse_pagination(Some("limit=1")).unwrap(),
            (1, String::new())
        );
        assert_eq!(
            parse_pagination(Some("limit=100")).unwrap(),
            (MAX_CONTAINER_LIMIT, String::new())
        );
        let id = "a".repeat(64);
        let cursor = format!("v1:{id}");
        assert_eq!(
            parse_pagination(Some(&format!("limit=10&cursor={cursor}"))).unwrap(),
            (10, cursor.clone())
        );
        // Empty cursor means the first page.
        assert_eq!(
            parse_pagination(Some("cursor=")).unwrap(),
            (50, String::new())
        );

        for invalid in [
            "limit=0",
            "limit=101",
            "limit=",
            "limit=abc",
            "limit=-1",
            "limit=+10",
            "limit=10.5",
            "limit=99999999999999999999",
            "limit=10&limit=10",
            "cursor=a&cursor=b",
            "unknown=1",
            "LIMIT=10",
            "cursor=not-a-cursor",
            "cursor=v1:short",
            &format!("cursor=v1:{}x", "b".repeat(64)),
            &format!("cursor={}", "x".repeat(MAX_CONTAINER_CURSOR_LEN + 1)),
            &format!("limit=10&cursor={}", "x".repeat(68)),
        ] {
            assert!(
                parse_pagination(Some(invalid)).is_err(),
                "{invalid} must be invalid_pagination"
            );
        }
        assert!(parse_limit("50").is_ok());
        assert!(is_valid_cursor(""));
        assert!(is_valid_cursor(&cursor));
        assert!(!is_valid_cursor("v1:short"));
    }

    #[test]
    fn agent_errors_map_to_stable_statuses_without_internals() {
        for (error, status) in [
            (AgentClientError::InvalidPagination, StatusCode::BAD_REQUEST),
            (AgentClientError::TimedOut, StatusCode::GATEWAY_TIMEOUT),
            (
                AgentClientError::Unavailable,
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                AgentClientError::IdentityMismatch,
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                AgentClientError::InvalidResponse,
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (AgentClientError::Cancelled, StatusCode::SERVICE_UNAVAILABLE),
        ] {
            assert_eq!(map_agent_error(error).status(), status, "{error:?}");
        }
        // Exact typed bodies are asserted in the authenticated HTTP tests;
        // transport detail never crosses there.
    }

    #[test]
    fn page_dto_preserves_order_and_exposes_only_allowlisted_fields() {
        let first_id = "a".repeat(64);
        let second_id = "b".repeat(64);
        let page = AgentContainerPage {
            containers: vec![summary(&first_id), summary(&second_id)],
            next_page_token: Some(format!("v1:{first_id}")),
        };
        assert_eq!(page.containers[0].container_id, first_id);
        assert_eq!(page.containers[1].container_id, second_id);

        let response = super::ContainerPageResponse {
            items: page
                .containers
                .into_iter()
                .map(|container| super::ContainerItemResponse {
                    container_id: container.container_id,
                    display_name: container.display_name,
                    image_reference: container.image_reference,
                    state: container.state.into(),
                    status: container.status,
                    created_at: chrono::DateTime::from_timestamp(container.created_at_seconds, 0)
                        .unwrap()
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                    node_id: container.node_id,
                })
                .collect(),
            next_cursor: page.next_page_token,
        };
        let value = serde_json::to_value(&response).unwrap();
        assert_eq!(
            value.as_object().unwrap().keys().collect::<Vec<_>>(),
            ["items", "nextCursor"],
            "page envelope is exactly items plus nextCursor"
        );
        let item = &value["items"][0];
        let keys: std::collections::BTreeSet<String> =
            item.as_object().unwrap().keys().cloned().collect();
        assert_eq!(
            keys,
            [
                "containerId",
                "createdAt",
                "displayName",
                "imageReference",
                "nodeId",
                "state",
                "status"
            ]
            .into_iter()
            .map(str::to_owned)
            .collect::<std::collections::BTreeSet<_>>(),
            "no environment, command, label, mount, port, or inspect field"
        );
        assert_eq!(item["state"], "running");
        assert_eq!(item["containerId"], first_id);
        let serialized = serde_json::to_string(&value).unwrap();
        for forbidden in [
            "command",
            "environment",
            "label",
            "mount",
            "network",
            "secret-canary",
            "/var/run/docker.sock",
        ] {
            assert!(!serialized.contains(forbidden), "{forbidden} leaked");
        }
    }

    #[test]
    fn state_mapping_covers_every_agent_state() {
        for (agent, public) in [
            (
                AgentContainerState::Unknown,
                ContainerStateResponse::Unknown,
            ),
            (
                AgentContainerState::Created,
                ContainerStateResponse::Created,
            ),
            (
                AgentContainerState::Running,
                ContainerStateResponse::Running,
            ),
            (AgentContainerState::Paused, ContainerStateResponse::Paused),
            (
                AgentContainerState::Restarting,
                ContainerStateResponse::Restarting,
            ),
            (
                AgentContainerState::Removing,
                ContainerStateResponse::Removing,
            ),
            (AgentContainerState::Exited, ContainerStateResponse::Exited),
            (AgentContainerState::Dead, ContainerStateResponse::Dead),
            (
                AgentContainerState::Stopping,
                ContainerStateResponse::Stopping,
            ),
        ] {
            assert_eq!(ContainerStateResponse::from(agent), public);
        }
    }

    // --- Authenticated HTTP integration: production route composition ---

    #[derive(Debug, Clone)]
    struct FakeAgent {
        node_id: String,
        calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        active: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        entered: std::sync::Arc<tokio::sync::Semaphore>,
        seen: std::sync::Arc<std::sync::Mutex<Vec<(u32, String)>>>,
        page: AgentContainerPage,
        error: Option<AgentClientError>,
        block: bool,
    }

    struct ActiveGuard<'a>(&'a std::sync::atomic::AtomicUsize);

    impl Drop for ActiveGuard<'_> {
        fn drop(&mut self) {
            self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    #[async_trait::async_trait]
    impl crate::rpc::AgentClient for FakeAgent {
        async fn describe(
            &self,
            _cancellation: &tokio_util::sync::CancellationToken,
        ) -> Result<crate::rpc::AgentDescription, AgentClientError> {
            Ok(crate::rpc::AgentDescription {
                node_id: self.node_id.clone(),
                agent_version: "test".to_owned(),
                architecture: "test".to_owned(),
                docker_connected: false,
            })
        }

        async fn list_containers(
            &self,
            page_size: u32,
            page_token: String,
            cancellation: &tokio_util::sync::CancellationToken,
        ) -> Result<AgentContainerPage, AgentClientError> {
            use std::sync::atomic::Ordering;
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.active.fetch_add(1, Ordering::SeqCst);
            let _guard = ActiveGuard(&self.active);
            self.entered.add_permits(1);
            self.seen
                .lock()
                .unwrap()
                .push((page_size, page_token.clone()));
            if self.block {
                cancellation.cancelled().await;
                return Err(AgentClientError::Cancelled);
            }
            if let Some(error) = self.error {
                return Err(error);
            }
            Ok(self.page.clone())
        }

        fn node_id(&self) -> &str {
            &self.node_id
        }
    }

    fn fake_page() -> AgentContainerPage {
        let first_id = "a".repeat(64);
        AgentContainerPage {
            containers: vec![summary(&first_id)],
            next_page_token: None,
        }
    }

    fn fake_agent(
        page: AgentContainerPage,
        error: Option<AgentClientError>,
        block: bool,
    ) -> (FakeAgent, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::sync::Arc;
        use std::sync::atomic::AtomicUsize;
        let calls = Arc::new(AtomicUsize::new(0));
        let agent = FakeAgent {
            node_id: "node-1".to_owned(),
            calls: calls.clone(),
            active: Arc::new(AtomicUsize::new(0)),
            entered: Arc::new(tokio::sync::Semaphore::new(0)),
            seen: Arc::new(std::sync::Mutex::new(Vec::new())),
            page,
            error,
            block,
        };
        (agent, calls)
    }

    async fn test_principal(
        database: &sea_orm::DatabaseConnection,
    ) -> Result<uuid::Uuid, Box<dyn std::error::Error>> {
        use sea_orm::{ActiveModelTrait, ActiveValue::Set};
        let now = chrono::Utc::now();
        let id = uuid::Uuid::now_v7();
        crate::auth::entities::principal::ActiveModel {
            id: Set(id),
            email: Set(None),
            display_name: Set(None),
            kind: Set("human".to_owned()),
            status: Set("active".to_owned()),
            authorization_epoch: Set(0),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(database)
        .await?;
        Ok(id)
    }

    async fn router_with_fake(
        connection: &sea_orm::DatabaseConnection,
        agent: FakeAgent,
    ) -> Result<
        (
            axum::Router,
            tokio_util::sync::CancellationToken,
            std::sync::Arc<FakeAgent>,
        ),
        Box<dyn std::error::Error>,
    > {
        use loco_rs::{
            app::{AppContext, Hooks},
            config::Config,
            environment::Environment,
        };
        let config = Config::from_folder(
            &Environment::Test,
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config"),
        )?;
        let context = crate::app::App::after_context(
            AppContext::builder(Environment::Test, connection.clone(), config).build(),
        )
        .await?;
        let shutdown = context
            .shared_store
            .get::<tokio_util::sync::CancellationToken>()
            .unwrap();
        let agent = std::sync::Arc::new(agent);
        context.shared_store.insert(crate::rpc::SharedAgentClient(
            agent.clone() as std::sync::Arc<dyn crate::rpc::AgentClient>
        ));
        let router = crate::app::App::routes(&context)
            .to_router::<crate::app::App>(context, axum::Router::new())?;
        Ok((router, shutdown, agent))
    }

    async fn production_router_without_client(
        connection: &sea_orm::DatabaseConnection,
    ) -> Result<axum::Router, Box<dyn std::error::Error>> {
        use loco_rs::{
            app::{AppContext, Hooks},
            config::Config,
            environment::Environment,
        };
        let config = Config::from_folder(
            &Environment::Test,
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config"),
        )?;
        let context = crate::app::App::after_context(
            AppContext::builder(Environment::Test, connection.clone(), config).build(),
        )
        .await?;
        Ok(crate::app::App::routes(&context)
            .to_router::<crate::app::App>(context, axum::Router::new())?)
    }

    #[tokio::test]
    async fn postgres_containers_serve_the_configured_node_only()
    -> Result<(), Box<dyn std::error::Error>> {
        use axum::{
            body::{Body, to_bytes},
            http::{Request, StatusCode, header},
        };
        use tower::ServiceExt;
        let Some(test_database) = crate::test_support::PostgresTestDatabase::create(true).await?
        else {
            return Ok(());
        };

        let evidence = async {
            let owner = test_principal(&test_database.connection).await?;
            let session =
                crate::auth::actions::create_session::execute(&test_database.connection, owner)
                    .await?;
            let authorization = format!("Bearer {}", session.access_plaintext);
            let (agent, calls) = fake_agent(fake_page(), None, false);
            let (router, _shutdown, fake) =
                router_with_fake(&test_database.connection, agent).await?;

            // Anonymous is denied before any handler or agent work.
            let anonymous = router
                .clone()
                .oneshot(Request::get("/api/v1/nodes/node-1/containers").body(Body::empty())?)
                .await?;
            let anonymous_status = anonymous.status();

            // Authenticated success preserves order, fields, and null cursor.
            let ok = router
                .clone()
                .oneshot(
                    Request::get("/api/v1/nodes/node-1/containers?limit=1")
                        .header(header::AUTHORIZATION, &authorization)
                        .body(Body::empty())?,
                )
                .await?;
            let ok_status = ok.status();
            let ok_body = to_bytes(ok.into_body(), 64 * 1024).await?;
            let ok_json: serde_json::Value = serde_json::from_slice(&ok_body)?;

            // The second page forwards the opaque cursor unchanged.
            let cursor = format!("v1:{}", "b".repeat(64));
            let second = router
                .clone()
                .oneshot(
                    Request::get(format!(
                        "/api/v1/nodes/node-1/containers?limit=2&cursor={cursor}"
                    ))
                    .header(header::AUTHORIZATION, &authorization)
                    .body(Body::empty())?,
                )
                .await?;
            let second_status = second.status();
            let seen = fake.seen.lock().unwrap().clone();

            // Unknown node and invalid pagination never reach the agent.
            let calls_before = calls.load(std::sync::atomic::Ordering::SeqCst);
            let unknown = router
                .clone()
                .oneshot(
                    Request::get("/api/v1/nodes/node-2/containers")
                        .header(header::AUTHORIZATION, &authorization)
                        .body(Body::empty())?,
                )
                .await?;
            let unknown_status = unknown.status();
            let unknown_body = to_bytes(unknown.into_body(), 1024).await?;
            let mut invalid_statuses = Vec::new();
            for query in [
                "limit=0",
                "limit=101",
                "limit=abc",
                "limit=1&limit=1",
                "cursor=not-a-cursor",
                "unknown=1",
            ] {
                let response = router
                    .clone()
                    .oneshot(
                        Request::get(format!("/api/v1/nodes/node-1/containers?{query}"))
                            .header(header::AUTHORIZATION, &authorization)
                            .body(Body::empty())?,
                    )
                    .await?;
                invalid_statuses.push(response.status());
            }
            let calls_after = calls.load(std::sync::atomic::Ordering::SeqCst);

            Ok::<_, Box<dyn std::error::Error>>((
                anonymous_status,
                ok_status,
                ok_json,
                second_status,
                seen,
                unknown_status,
                unknown_body,
                invalid_statuses,
                calls_before,
                calls_after,
                fake.active.load(std::sync::atomic::Ordering::SeqCst),
            ))
        }
        .await;
        let removed = test_database.cleanup().await?;
        let (
            anonymous_status,
            ok_status,
            ok_json,
            second_status,
            seen,
            unknown_status,
            unknown_body,
            invalid_statuses,
            calls_before,
            calls_after,
            active,
        ) = evidence?;

        assert_eq!(anonymous_status, StatusCode::UNAUTHORIZED);
        assert_eq!(ok_status, StatusCode::OK);
        assert_eq!(ok_json["items"].as_array().unwrap().len(), 1);
        assert_eq!(ok_json["items"][0]["containerId"], "a".repeat(64));
        assert_eq!(ok_json["items"][0]["nodeId"], "node-1");
        assert_eq!(ok_json["items"][0]["state"], "running");
        assert!(ok_json.get("nextCursor").is_some());
        assert!(ok_json["nextCursor"].is_null());
        assert_eq!(second_status, StatusCode::OK);
        let cursor = format!("v1:{}", "b".repeat(64));
        assert_eq!(
            seen,
            vec![(1, String::new()), (2, cursor)],
            "HTTP pagination must reach the agent unchanged"
        );
        assert_eq!(unknown_status, StatusCode::NOT_FOUND);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&unknown_body)?,
            serde_json::json!({"error": "not_found"})
        );
        assert!(
            invalid_statuses
                .into_iter()
                .all(|status| status == StatusCode::BAD_REQUEST),
            "every malformed page is invalid_pagination"
        );
        // Two successes plus zero outbound calls for the unknown node and
        // six invalid pages.
        assert_eq!(calls_before, 2);
        assert_eq!(calls_after, 2);
        assert_eq!(active, 0, "no detached agent work remains");
        assert!(removed);
        Ok(())
    }

    #[tokio::test]
    async fn postgres_containers_map_agent_failures_without_internals()
    -> Result<(), Box<dyn std::error::Error>> {
        use axum::{
            body::{Body, to_bytes},
            http::{Request, StatusCode, header},
        };
        use tower::ServiceExt;
        let Some(test_database) = crate::test_support::PostgresTestDatabase::create(true).await?
        else {
            return Ok(());
        };

        let evidence = async {
            let owner = test_principal(&test_database.connection).await?;
            let session =
                crate::auth::actions::create_session::execute(&test_database.connection, owner)
                    .await?;
            let authorization = format!("Bearer {}", session.access_plaintext);
            let mut outcomes = Vec::new();
            for (error, page) in [
                (Some(AgentClientError::Unavailable), fake_page()),
                (Some(AgentClientError::TimedOut), fake_page()),
                (Some(AgentClientError::InvalidPagination), fake_page()),
                (Some(AgentClientError::InvalidResponse), fake_page()),
                (
                    None,
                    AgentContainerPage {
                        containers: vec![AgentContainerSummary {
                            created_at_seconds: i64::MAX,
                            ..summary(&"b".repeat(64))
                        }],
                        next_page_token: None,
                    },
                ),
                (
                    None,
                    AgentContainerPage {
                        containers: vec![AgentContainerSummary {
                            created_at_seconds: -1,
                            ..summary(&"b".repeat(64))
                        }],
                        next_page_token: None,
                    },
                ),
            ] {
                let (agent, _) = fake_agent(page, error, false);
                let (router, _, _) = router_with_fake(&test_database.connection, agent).await?;
                let response = router
                    .oneshot(
                        Request::get("/api/v1/nodes/node-1/containers")
                            .header(header::AUTHORIZATION, &authorization)
                            .body(Body::empty())?,
                    )
                    .await?;
                let status = response.status();
                let body = to_bytes(response.into_body(), 4096).await?;
                outcomes.push((status, body));
            }

            // No configured client fails closed as unavailable.
            let bare = production_router_without_client(&test_database.connection).await?;
            let missing = bare
                .oneshot(
                    Request::get("/api/v1/nodes/node-1/containers")
                        .header(header::AUTHORIZATION, &authorization)
                        .body(Body::empty())?,
                )
                .await?;
            let missing_status = missing.status();
            let missing_body = to_bytes(missing.into_body(), 4096).await?;

            Ok::<_, Box<dyn std::error::Error>>((outcomes, missing_status, missing_body))
        }
        .await;
        let removed = test_database.cleanup().await?;
        let (outcomes, missing_status, missing_body) = evidence?;

        let expected = [
            (StatusCode::SERVICE_UNAVAILABLE, "agent_unavailable"),
            (StatusCode::GATEWAY_TIMEOUT, "agent_timeout"),
            (StatusCode::BAD_REQUEST, "invalid_pagination"),
            (StatusCode::SERVICE_UNAVAILABLE, "agent_unavailable"),
            (StatusCode::SERVICE_UNAVAILABLE, "agent_unavailable"),
            (StatusCode::SERVICE_UNAVAILABLE, "agent_unavailable"),
        ];
        assert_eq!(outcomes.len(), expected.len());
        for ((status, body), (expected_status, code)) in outcomes.iter().zip(expected) {
            assert_eq!(*status, expected_status);
            let json: serde_json::Value = serde_json::from_slice(body)?;
            assert_eq!(json, serde_json::json!({"error": code}));
            let text = String::from_utf8_lossy(body);
            for canary in [
                "secret-canary",
                "/var/run/docker.sock",
                "127.0.0.1",
                "transport",
            ] {
                assert!(!text.contains(canary), "{canary} leaked in {code}");
            }
        }
        assert_eq!(missing_status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&missing_body)?,
            serde_json::json!({"error": "agent_unavailable"})
        );
        assert!(removed);
        Ok(())
    }

    #[tokio::test]
    async fn postgres_container_cancellation_joins_without_detached_work()
    -> Result<(), Box<dyn std::error::Error>> {
        use axum::{
            body::Body,
            http::{Request, header},
        };
        use tower::ServiceExt;
        let Some(test_database) = crate::test_support::PostgresTestDatabase::create(true).await?
        else {
            return Ok(());
        };

        let evidence = async {
            let owner = test_principal(&test_database.connection).await?;
            let session =
                crate::auth::actions::create_session::execute(&test_database.connection, owner)
                    .await?;
            let authorization = format!("Bearer {}", session.access_plaintext);
            let (agent, _) = fake_agent(fake_page(), None, true);
            let (router, _, fake) = router_with_fake(&test_database.connection, agent).await?;
            let call = tokio::spawn(async move {
                router
                    .oneshot(
                        Request::get("/api/v1/nodes/node-1/containers")
                            .header(header::AUTHORIZATION, &authorization)
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
            });

            fake.entered.acquire().await.unwrap().forget();
            call.abort();
            // Dropping the HTTP future drops the awaited client future; the
            // fake's active guard must return to zero without detached work.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let active = fake.active.load(std::sync::atomic::Ordering::SeqCst);
            Ok::<_, Box<dyn std::error::Error>>(active)
        }
        .await;
        let removed = test_database.cleanup().await?;
        let active = evidence?;

        assert_eq!(active, 0, "aborted HTTP request left agent work behind");
        assert!(removed);
        Ok(())
    }

    #[tokio::test]
    async fn postgres_openapi_serves_the_checked_artifact() -> Result<(), Box<dyn std::error::Error>>
    {
        use axum::{
            body::{Body, to_bytes},
            http::{Request, StatusCode, header},
        };
        use tower::ServiceExt;
        let Some(test_database) = crate::test_support::PostgresTestDatabase::create(true).await?
        else {
            return Ok(());
        };

        let evidence = async {
            let owner = test_principal(&test_database.connection).await?;
            let session =
                crate::auth::actions::create_session::execute(&test_database.connection, owner)
                    .await?;
            let authorization = format!("Bearer {}", session.access_plaintext);
            let router = production_router_without_client(&test_database.connection).await?;
            let anonymous = router
                .clone()
                .oneshot(Request::get("/api/v1/openapi.json").body(Body::empty())?)
                .await?
                .status();
            let ok = router
                .oneshot(
                    Request::get("/api/v1/openapi.json")
                        .header(header::AUTHORIZATION, &authorization)
                        .body(Body::empty())?,
                )
                .await?;
            let status = ok.status();
            let body = to_bytes(ok.into_body(), 256 * 1024).await?;
            let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(super::super::status::OPENAPI_ARTIFACT);
            let checked = std::fs::read(&manifest)?;
            Ok::<_, Box<dyn std::error::Error>>((anonymous, status, body, checked))
        }
        .await;
        let removed = test_database.cleanup().await?;
        let (anonymous, status, body, checked) = evidence?;

        assert_eq!(anonymous, StatusCode::UNAUTHORIZED);
        assert_eq!(status, StatusCode::OK);
        // Runtime serves the checked file semantically; formatting may differ
        // because `format::json` serializes compactly while the artifact is
        // pretty-printed. The golden test enforces byte equality separately.
        let served: serde_json::Value = serde_json::from_slice(&body)?;
        let checked_json: serde_json::Value = serde_json::from_slice(&checked)?;
        assert_eq!(served, checked_json, "runtime serves the checked file");
        assert!(served["paths"]["/api/v1/nodes/{nodeId}/containers"].is_object());
        assert!(removed);
        Ok(())
    }
}
