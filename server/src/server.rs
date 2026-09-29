use std::{
    future::Future,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

use axum::{
    Router,
    extract::Request,
    middleware::{self, Next},
    response::Response,
};
use hyper::server::conn::http1;
use hyper_util::{rt::TokioIo, server::conn::auto, service::TowerToHyperService};
use kontainer_engine::DockerHost;
use kontainer_lifecycle::{DrainClock, ShutdownGrace};
use kontainer_rpc_auth::{MutualTlsIdentityFiles, spawn_tracked, valid_identity_segment};
use loco_rs::{
    Error, Result,
    app::AppContext,
    boot::{ServeParams, shutdown_signal},
    environment::Environment,
};
use serde::Deserialize;
use tokio::{net::TcpListener, sync::mpsc::UnboundedSender, task::JoinSet};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::{
    auth::github::{
        self, ClientSecret, GitHubAuthService, GitHubAuthSettings, GitHubLoginConfig,
        ReqwestGitHubProvider,
    },
    rpc::serve_controller_rpc,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ControllerSettings {
    agent_rpc: AgentRpcSettings,
    public_http: PublicHttpSettings,
    #[serde(default)]
    docker: DockerSettings,
    #[serde(default, deserialize_with = "deserialize_shutdown_grace")]
    shutdown_grace: ShutdownGrace,
    #[serde(default)]
    github_auth: GitHubAuthSettings,
}

/// The Docker Engine the controller's manager-scoped engine talks to
/// (spec 10). `tcp://` is the dev-cluster override from ADR 0004 and is
/// refused outside development and test by [`validate_docker_host`].
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockerSettings {
    #[serde(deserialize_with = "deserialize_docker_host")]
    pub(crate) host: DockerHost,
}

fn deserialize_docker_host<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<DockerHost, D::Error> {
    String::deserialize(deserializer)?
        .parse()
        .map_err(serde::de::Error::custom)
}

/// The manager-scoped Docker host, validated for the running environment.
#[derive(Debug, Clone)]
pub struct ManagerDockerHost(pub DockerHost);

fn validate_docker_host(docker: &DockerSettings, environment: &Environment) -> Result<()> {
    if docker.host.is_tcp() && !matches!(environment, Environment::Development | Environment::Test)
    {
        return Err(Error::Message(format!(
            "settings.docker.host may use tcp:// only in development or test, not {environment:?}"
        )));
    }
    Ok(())
}

pub fn init_docker_state(ctx: &AppContext) -> Result<()> {
    let settings = controller_settings(ctx)?;
    validate_docker_host(&settings.docker, &ctx.environment)?;
    ctx.shared_store
        .insert(ManagerDockerHost(settings.docker.host));
    Ok(())
}

fn deserialize_shutdown_grace<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<ShutdownGrace, D::Error> {
    let value = String::deserialize(deserializer)?;
    value.parse().map_err(serde::de::Error::custom)
}

#[derive(Clone)]
pub struct ControllerShutdown {
    draining: CancellationToken,
    force: CancellationToken,
    clock: Arc<DrainClock>,
}

impl ControllerShutdown {
    pub fn new(grace: ShutdownGrace) -> Self {
        Self {
            draining: CancellationToken::new(),
            force: CancellationToken::new(),
            clock: Arc::new(DrainClock::new(grace)),
        }
    }

    pub fn begin_drain(&self) -> Instant {
        let deadline = self.clock.begin();
        self.draining.cancel();
        deadline
    }

    pub fn draining(&self) -> CancellationToken {
        self.draining.clone()
    }

    pub fn force(&self) -> CancellationToken {
        self.force.clone()
    }
}

impl From<CancellationToken> for ControllerShutdown {
    fn from(draining: CancellationToken) -> Self {
        Self {
            draining,
            force: CancellationToken::new(),
            clock: Arc::new(DrainClock::new(ShutdownGrace::default())),
        }
    }
}

pub fn init_shutdown_state(ctx: &AppContext) -> Result<()> {
    let grace = controller_settings(ctx)?.shutdown_grace;
    let shutdown = ControllerShutdown::new(grace);
    // Existing probe and admission state continues to use the drain token.
    ctx.shared_store.insert(shutdown.draining());
    ctx.shared_store.insert(shutdown.clone());
    ctx.shared_store.insert(ForceCancellation(shutdown.force()));
    Ok(())
}

#[derive(Clone)]
pub struct ForceCancellation(pub CancellationToken);

/// Struct shape (not a tagged enum): serde's `deny_unknown_fields` is not
/// honored for internally tagged enums, so the mode/bundle consistency is
/// enforced by [`PublicHttpSettings::validate`] instead.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicHttpSettings {
    mode: PublicHttpMode,
    tls: Option<PublicTlsSettings>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum PublicHttpMode {
    Tls,
    Plaintext,
}

impl PublicHttpSettings {
    /// Reject a TLS mode without a bundle and a plaintext mode with one.
    /// Runs at settings load, before any listener binds.
    fn validate(&self) -> Result<()> {
        match (self.mode, &self.tls) {
            (PublicHttpMode::Tls, Some(_)) | (PublicHttpMode::Plaintext, None) => Ok(()),
            (PublicHttpMode::Tls, None) => Err(Error::string(
                "public_http mode \"tls\" requires a tls certificate/key bundle",
            )),
            (PublicHttpMode::Plaintext, Some(_)) => Err(Error::string(
                "public_http mode \"plaintext\" forbids a tls certificate/key bundle",
            )),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicTlsSettings {
    pub certificate_path: PathBuf,
    pub private_key_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentRpcSettings {
    enabled: bool,
    binding: SocketAddr,
    #[serde(deserialize_with = "deserialize_cluster_id")]
    cluster_id: String,
    tls: AgentRpcTlsSettings,
    /// Agent certificates carry `{node_id}.{suffix}` as their DNS name.
    #[serde(
        default = "default_agent_server_name_suffix",
        deserialize_with = "deserialize_dns_suffix"
    )]
    agent_server_name_suffix: String,
}

fn default_agent_server_name_suffix() -> String {
    crate::nodes::agents::DEFAULT_AGENT_SERVER_NAME_SUFFIX.to_owned()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentRpcTlsSettings {
    certificate_path: PathBuf,
    private_key_path: PathBuf,
    client_ca_path: PathBuf,
}

#[derive(Debug, Clone, Copy)]
enum ServerKind {
    Http,
    Https,
    AgentRpc,
}

impl std::fmt::Display for ServerKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http => formatter.write_str("HTTP"),
            Self::Https => formatter.write_str("HTTPS"),
            Self::AgentRpc => formatter.write_str("agent RPC"),
        }
    }
}

pub async fn serve(router: Router, ctx: &AppContext, params: &ServeParams) -> Result<()> {
    let settings = controller_settings(ctx)?;
    let http_listener = TcpListener::bind(format!("{}:{}", params.binding, params.port)).await?;
    // Validate the public transport before any listener admits handlers: a bad
    // TLS identity or a non-loopback plaintext request fails startup here.
    let transport = resolve_public_transport(
        &settings.public_http,
        &ctx.environment,
        http_listener.local_addr().map_err(Error::wrap)?,
        &ctx.config.server.host,
    )
    .await?;
    let agent_rpc = if settings.agent_rpc.enabled {
        let recorder = crate::rpc::PostgresCheckInRecorder::new(
            ctx.db.clone(),
            settings.agent_rpc.cluster_id.clone(),
        );
        Some(BoundAgentRpcServer {
            listener: TcpListener::bind(settings.agent_rpc.binding).await?,
            cluster_id: settings.agent_rpc.cluster_id,
            tls_files: settings.agent_rpc.tls.into(),
            intake: crate::rpc::ControllerIntakeService::recording(Arc::new(recorder)),
        })
    } else {
        None
    };
    let shutdown = ctx
        .shared_store
        .get::<ControllerShutdown>()
        .ok_or_else(|| Error::string("controller lifecycle is not initialized"))?;
    let servers = supervise_controller_servers(
        router,
        BoundHttpServer {
            listener: http_listener,
            transport,
        },
        agent_rpc,
        shutdown.clone(),
    );
    tokio::pin!(servers);
    // Swarm-wide objects go through the configured manager; without one, manager-scoped routes
    // answer 503 `not_a_manager` and the rest keeps working.
    if let Some(host) = ctx.shared_store.get::<ManagerDockerHost>()
        && let Some(manager) = crate::swarm::engine::connect_manager(&host.0).await
    {
        ctx.shared_store
            .insert(crate::swarm::engine::SharedManagerEngine(manager));
    }
    // Hourly sample pruning stops with the drain and is joined before serve returns.
    let pruner = tokio::spawn(crate::tasks::prune_samples::run_hourly(
        ctx.db.clone(),
        shutdown.draining(),
    ));
    // The operation worker recovers interrupted operations first, then claims until the drain,
    // and gets the shutdown grace to finish in-flight executions (ADR 0009).
    let worker = tokio::spawn(
        crate::operations::worker::Worker {
            database: ctx.db.clone(),
            registry: Arc::new(crate::operations::executor::production_registry()),
            agents: ctx
                .shared_store
                .get::<crate::nodes::agents::SharedAgentDirectory>()
                .map(|directory| directory.0),
            manager: ctx
                .shared_store
                .get::<crate::swarm::engine::SharedManagerEngine>()
                .map(|manager| manager.0),
            settings: crate::operations::worker::WorkerSettings::new(
                settings.shutdown_grace.duration(),
            ),
        }
        .run(shutdown.draining()),
    );

    // Log streams close with 1001 when the drain starts; upgraded connections are not HTTP
    // requests the servers track.
    let log_streams = ctx.shared_store.get::<crate::logs::LogStreams>();
    let draining = shutdown.draining();
    let streams = tokio::spawn(async move {
        draining.cancelled().await;
        if let Some(streams) = log_streams
            && let Err(error) = streams.server.shutdown().await
        {
            tracing::warn!(?error, "log streams did not close within the grace");
        }
    });

    let result = tokio::select! {
        result = &mut servers => result,
        _ = shutdown_signal() => {
            shutdown.begin_drain();
            servers.await
        }
    };
    shutdown.begin_drain();
    if let Err(error) = streams.await {
        tracing::error!(%error, "log stream shutdown task failed");
    }
    if let Err(error) = pruner.await {
        tracing::error!(%error, "sample pruning task failed");
    }
    if let Err(error) = worker.await {
        tracing::error!(%error, "operation worker task failed");
    }
    result
}

pub struct BoundAgentRpcServer {
    pub listener: TcpListener,
    pub cluster_id: String,
    pub tls_files: MutualTlsIdentityFiles,
    pub intake: crate::rpc::ControllerIntakeService,
}

pub struct BoundHttpServer {
    pub listener: TcpListener,
    pub transport: HttpTransport,
}

pub enum HttpTransport {
    Plaintext,
    Tls(Arc<rustls::ServerConfig>),
}

impl std::fmt::Debug for HttpTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plaintext => formatter.write_str("Plaintext"),
            Self::Tls(_) => formatter.write_str("Tls(..)"),
        }
    }
}

#[derive(Clone)]
struct TrackedExecutor {
    tasks: TaskTracker,
    force: CancellationToken,
    failures: UnboundedSender<&'static str>,
}

impl<F> hyper::rt::Executor<F> for TrackedExecutor
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    fn execute(&self, future: F) {
        let force = self.force.clone();
        spawn_tracked(
            &self.tasks,
            &self.failures,
            "public HTTP request",
            async move {
                tokio::select! {
                    _ = force.cancelled() => {}
                    _ = future => {}
                }
            },
        );
    }
}

/// Listener tasks stop admission; the supervisor owns all connection tasks.
async fn serve_tls(
    listener: TcpListener,
    router: Router,
    config: Arc<rustls::ServerConfig>,
    cancellation: CancellationToken,
    force: CancellationToken,
    connections: TaskTracker,
    failures: UnboundedSender<&'static str>,
) -> Result<ServerKind> {
    let acceptor = tokio_rustls::TlsAcceptor::from(config);
    loop {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => break,
            accepted = listener.accept() => {
                let (socket, peer) = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) => return Err(Error::wrap(error)),
                };
                let router = router.clone();
                let acceptor = acceptor.clone();
                let connection_cancel = cancellation.clone();
                let connection_force = force.clone();
                spawn_tracked(&connections, &failures, "public HTTPS connection", async move {
                    tokio::select! {
                        biased;
                        _ = connection_force.cancelled() => {}
                        _ = serve_tls_connection(router, acceptor, socket, peer, connection_cancel) => {}
                    }
                });
            }
        }
    }
    Ok(ServerKind::Https)
}

async fn serve_plain(
    listener: TcpListener,
    router: Router,
    draining: CancellationToken,
    force: CancellationToken,
    connections: TaskTracker,
    requests: TaskTracker,
    failures: UnboundedSender<&'static str>,
) -> Result<ServerKind> {
    loop {
        let (socket, peer) = tokio::select! {
            biased;
            _ = draining.cancelled() => return Ok(ServerKind::Http),
            accepted = listener.accept() => accepted.map_err(Error::wrap)?,
        };
        let router = router.clone();
        let connection_draining = draining.clone();
        let connection_force = force.clone();
        let requests = requests.clone();
        let request_failures = failures.clone();
        spawn_tracked(
            &connections,
            &failures,
            "public HTTP connection",
            async move {
                tokio::select! {
                    biased;
                    _ = connection_force.cancelled() => {}
                    _ = serve_plain_connection(socket, peer, router, connection_draining, connection_force.clone(), requests, request_failures) => {}
                }
            },
        );
    }
}

async fn serve_plain_connection(
    socket: tokio::net::TcpStream,
    peer: SocketAddr,
    router: Router,
    draining: CancellationToken,
    force: CancellationToken,
    requests: TaskTracker,
    failures: UnboundedSender<&'static str>,
) {
    use tower::Service as _;

    let mut make_service = router.into_make_service_with_connect_info::<SocketAddr>();
    let service = match make_service.call(peer).await {
        Ok(service) => service,
        Err(error) => match error {},
    };
    let builder = auto::Builder::new(TrackedExecutor {
        tasks: requests,
        force,
        failures,
    });
    let connection = builder
        .serve_connection_with_upgrades(TokioIo::new(socket), TowerToHyperService::new(service));
    tokio::pin!(connection);
    tokio::select! {
        biased;
        _ = draining.cancelled() => {
            connection.as_mut().graceful_shutdown();
            let _ = connection.await;
        }
        _ = &mut connection => {}
    }
}

async fn serve_tls_connection(
    router: Router,
    acceptor: tokio_rustls::TlsAcceptor,
    socket: tokio::net::TcpStream,
    peer: SocketAddr,
    cancellation: CancellationToken,
) {
    use tower::Service as _;

    let accepted = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return,
        accepted = acceptor.accept(socket) => accepted,
    };
    let Ok(stream) = accepted else {
        // Plaintext bytes or an untrusted client: rejected before any handler.
        return;
    };
    let mut make_service = router.into_make_service_with_connect_info::<SocketAddr>();
    let service = match make_service.call(peer).await {
        Ok(service) => service,
        Err(error) => match error {},
    };
    let connection = http1::Builder::new()
        .serve_connection(TokioIo::new(stream), TowerToHyperService::new(service));
    tokio::pin!(connection);
    // Connection errors are connection-local: a broken client must not fail
    // the server. Cancellation drains admitted requests first.
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            connection.as_mut().graceful_shutdown();
            let _ = connection.await;
        }
        _ = &mut connection => {}
    }
}

pub async fn supervise_controller_servers(
    router: Router,
    http: BoundHttpServer,
    agent_rpc: Option<BoundAgentRpcServer>,
    shutdown: impl Into<ControllerShutdown>,
) -> Result<()> {
    let shutdown = shutdown.into();
    let cancellation = shutdown.draining();
    let force = shutdown.force();
    let http_connections = TaskTracker::new();
    let http_requests = TaskTracker::new();
    let rpc_connections = TaskTracker::new();
    let rpc_requests = TaskTracker::new();
    let (failures, mut task_failures) = tokio::sync::mpsc::unbounded_channel();
    let agent_rpc = agent_rpc
        .map(|agent_rpc| {
            serve_controller_rpc(
                agent_rpc.listener,
                agent_rpc.cluster_id,
                agent_rpc.tls_files,
                agent_rpc.intake,
                cancellation.clone(),
                force.clone(),
                rpc_connections.clone(),
                rpc_requests.clone(),
                failures.clone(),
            )
        })
        .transpose()
        .map_err(|error| {
            shutdown.begin_drain();
            Error::Message(format!("agent RPC server failed: {error}"))
        })?;
    let mut servers = JoinSet::new();

    if cancellation.is_cancelled() {
        shutdown.begin_drain();
        return Ok(());
    }
    let router = router
        .layer(middleware::from_fn_with_state(
            cancellation.clone(),
            crate::controllers::health::admit_request,
        ))
        // Path-only access log. Loco's `logger` middleware records the full
        // URI including query strings, which would persist OAuth `code` and
        // `state` in diagnostics; it stays disabled in checked-in configs.
        .layer(middleware::from_fn(path_only_request_log));

    match http.transport {
        HttpTransport::Plaintext => {
            servers.spawn(serve_plain(
                http.listener,
                router,
                cancellation.clone(),
                force.clone(),
                http_connections.clone(),
                http_requests.clone(),
                failures.clone(),
            ));
        }
        HttpTransport::Tls(config) => {
            servers.spawn(serve_tls(
                http.listener,
                router,
                config,
                cancellation.clone(),
                force.clone(),
                http_connections.clone(),
                failures.clone(),
            ));
        }
    }

    if let Some(agent_rpc) = agent_rpc {
        servers.spawn(async move {
            agent_rpc
                .await
                .map_err(|error| Error::Message(format!("agent RPC server failed: {error}")))?;

            Ok(ServerKind::AgentRpc)
        });
    }

    let mut failure = tokio::select! {
        biased;
        _ = cancellation.cancelled() => None,
        completion = servers.join_next() => completion.map(unexpected_server_completion),
        kind = task_failures.recv() => kind.map(|kind| Error::Message(format!("{kind} task failed"))),
    };
    let deadline = shutdown.begin_drain();

    let graceful = async {
        while let Some(completion) = servers.join_next().await {
            if failure.is_none()
                && let Err(error) = completed_server(completion)
            {
                failure = Some(error);
            }
        }
        http_connections.close();
        http_requests.close();
        rpc_connections.close();
        rpc_requests.close();
        tokio::join!(
            http_connections.wait(),
            http_requests.wait(),
            rpc_connections.wait(),
            rpc_requests.wait(),
        );
    };

    if tokio::time::timeout_at(deadline.into(), graceful)
        .await
        .is_err()
    {
        tracing::error!(
            role = "controller",
            public_http_connections = http_connections.len(),
            public_http_requests = http_requests.len(),
            agent_rpc_connections = rpc_connections.len(),
            agent_rpc_requests = rpc_requests.len(),
            listener_tasks = servers.len(),
            "shutdown deadline exceeded"
        );
        force.cancel();
        servers.shutdown().await;
        http_connections.close();
        http_requests.close();
        rpc_connections.close();
        rpc_requests.close();
        tokio::join!(
            http_connections.wait(),
            http_requests.wait(),
            rpc_connections.wait(),
            rpc_requests.wait(),
        );
        return Err(
            failure.unwrap_or_else(|| Error::string("controller shutdown deadline exceeded"))
        );
    }

    if failure.is_none()
        && let Ok(kind) = task_failures.try_recv()
    {
        failure = Some(Error::Message(format!("{kind} task failed")));
    }

    failure.map_or(Ok(()), Err)
}

fn controller_settings(ctx: &AppContext) -> Result<ControllerSettings> {
    let settings = ctx
        .config
        .settings
        .clone()
        .ok_or_else(|| Error::string("controller settings are missing"))?;

    parse_controller_settings(settings)
}

fn parse_controller_settings(settings: serde_json::Value) -> Result<ControllerSettings> {
    serde_json::from_value(settings)
        .map_err(|_| {
            Error::string(
                "invalid controller settings: check agent_rpc, public_http, docker, github_auth, shutdown_grace, and cluster_id keys and types",
            )
        })
        .and_then(|settings: ControllerSettings| {
            settings.public_http.validate()?;
            settings
                .github_auth
                .validate()
                .map_err(|message| Error::string(&message))?;
            Ok(settings)
        })
}

/// Method/path/status access log. The query string is never recorded:
/// OAuth callback parameters must not reach tracing output.
async fn path_only_request_log(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let response = next.run(request).await;
    tracing::info!(
        http.method = %method,
        http.path = %path,
        http.status = response.status().as_u16(),
        "http-request"
    );
    response
}

/// Cluster identity carried in login responses. Inserted once in
/// [`init_shared_auth_state`] from the static agent RPC settings.
#[derive(Debug, Clone)]
pub struct ControllerClusterId(pub String);

/// Build the GitHub login service and cluster identity from validated
/// settings. A missing secret file or invalid origin fails startup when
/// login is enabled; a disabled service fails login requests closed.
///
/// With agent RPC enabled, the per-node agent directory is inserted too: it
/// dials each node at its advertised address with the controller identity.
pub fn init_shared_auth_state(ctx: &AppContext) -> Result<()> {
    use std::sync::Arc;

    let settings = controller_settings(ctx)?;
    ctx.shared_store
        .insert(ControllerClusterId(settings.agent_rpc.cluster_id.clone()));
    ctx.shared_store
        .insert(crate::auth::throttle::LoginThrottle::default());

    if settings.agent_rpc.enabled {
        let directory = crate::nodes::agents::MtlsAgentDirectory::new(
            settings.agent_rpc.cluster_id.clone(),
            settings.agent_rpc.tls.clone().into(),
            settings.agent_rpc.agent_server_name_suffix.clone(),
        );
        ctx.shared_store
            .insert(crate::nodes::agents::SharedAgentDirectory(Arc::new(
                directory,
            )));
    }

    let service = if settings.github_auth.enabled {
        let secret_path = settings
            .github_auth
            .secret_path()
            .ok_or_else(|| Error::string("github_auth client secret is unavailable"))?;
        let raw = std::fs::read_to_string(secret_path)
            .map_err(|_| Error::string("github_auth client secret is unavailable"))?;
        let secret = raw.trim_end_matches(['\n', '\r']);
        if secret.is_empty() {
            return Err(Error::string("github_auth client secret is unavailable"));
        }
        github::callback_url(&ctx.config.server.host).map_err(|message| Error::string(&message))?;
        let allowlisted = settings
            .github_auth
            .allowlist()
            .map_err(|message| Error::string(&message))?;
        let provider = ReqwestGitHubProvider::new(
            settings.github_auth.client_id.clone(),
            ClientSecret::new(secret.to_owned()),
            github::GITHUB_TOKEN_URL.to_owned(),
            github::GITHUB_USER_URL.to_owned(),
        )
        .map_err(|_| Error::string("github_auth provider is unavailable"))?;
        GitHubAuthService {
            config: GitHubLoginConfig {
                enabled: true,
                client_id: settings.github_auth.client_id.clone(),
                allowlisted,
                authorize_url: github::GITHUB_AUTHORIZE_URL.to_owned(),
                token_url: github::GITHUB_TOKEN_URL.to_owned(),
                user_url: github::GITHUB_USER_URL.to_owned(),
            },
            provider: Arc::new(provider),
        }
    } else {
        GitHubAuthService::disabled()
    };
    ctx.shared_store.insert(service);
    Ok(())
}

fn deserialize_cluster_id<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<String, D::Error> {
    let value = String::deserialize(deserializer)?;
    if !valid_identity_segment(&value) {
        return Err(serde::de::Error::custom(
            "cluster_id must be a canonical workload identity segment",
        ));
    }
    Ok(value)
}

fn deserialize_dns_suffix<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<String, D::Error> {
    let value = String::deserialize(deserializer)?;
    let valid = !value.is_empty()
        && value.len() <= 200
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
        });
    if !valid {
        return Err(serde::de::Error::custom(
            "agent_server_name_suffix must be a DNS name",
        ));
    }
    Ok(value)
}

/// Load and validate the public TLS identity before any listener admits
/// handlers. The ring provider is selected explicitly: provider auto-detect
/// panics, and must stay out of every construction path.
pub fn load_public_tls_config(settings: &PublicTlsSettings) -> Result<Arc<rustls::ServerConfig>> {
    use rustls::pki_types::pem::PemObject;

    let certificate = std::fs::read(&settings.certificate_path)
        .map_err(|error| tls_identity_error(settings, &error.to_string()))?;
    let private_key = std::fs::read(&settings.private_key_path)
        .map_err(|error| tls_identity_error(settings, &error.to_string()))?;
    // Load the whole chain: production bundles are leaf-plus-intermediate
    // files, and truncating them breaks clients that trust only the root.
    let chain: Vec<_> = rustls::pki_types::CertificateDer::pem_slice_iter(&certificate)
        .collect::<Result<_, _>>()
        .map_err(|error| tls_identity_error(settings, &error.to_string()))?;
    if chain.is_empty() {
        return Err(tls_identity_error(settings, "no certificate found"));
    }
    let private_key = rustls::pki_types::PrivateKeyDer::from_pem_slice(&private_key)
        .map_err(|error| tls_identity_error(settings, &error.to_string()))?;
    // `with_single_cert` rejects a key that does not match the leaf
    // certificate here, not on the first handshake.
    rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_protocol_versions(rustls::ALL_VERSIONS)
        .map_err(|error| tls_identity_error(settings, &error.to_string()))?
        .with_no_client_auth()
        .with_single_cert(chain, private_key)
        .map(Arc::new)
        .map_err(|error| tls_identity_error(settings, &error.to_string()))
}

/// File paths aid the operator; file contents and parser input never do.
fn tls_identity_error(settings: &PublicTlsSettings, detail: &str) -> Error {
    Error::Message(format!(
        "invalid public TLS identity ({} / {}): {detail}",
        settings.certificate_path.display(),
        settings.private_key_path.display()
    ))
}
/// Load and validate the public HTTP transport before any listener admits
/// handlers. TLS identities are proven by loading the certificate/key pair;
/// plaintext is confined to Development/Test on loopback only.
async fn resolve_public_transport(
    settings: &PublicHttpSettings,
    environment: &Environment,
    bind: SocketAddr,
    advertised_host: &str,
) -> Result<HttpTransport> {
    // Shape (()TLS with bundle, plaintext without) was already enforced by
    // `PublicHttpSettings::validate` at settings load; match the mode here.
    match settings.mode {
        PublicHttpMode::Tls => {
            let tls = settings.tls.as_ref().ok_or_else(|| {
                Error::string("public_http mode \"tls\" requires a tls certificate/key bundle")
            })?;
            load_public_tls_config(tls).map(HttpTransport::Tls)
        }
        PublicHttpMode::Plaintext => {
            if !matches!(environment, Environment::Development | Environment::Test) {
                return Err(Error::Message(format!(
                    "plaintext public HTTP is not allowed in the {environment:?} environment; configure TLS"
                )));
            }
            if !bind.ip().is_loopback() {
                return Err(Error::Message(format!(
                    "plaintext public HTTP must bind loopback, got {bind}"
                )));
            }
            if !advertised_url_is_loopback_http(advertised_host) {
                return Err(Error::Message(format!(
                    "plaintext public HTTP must advertise a loopback http URL, got {advertised_host}"
                )));
            }
            Ok(HttpTransport::Plaintext)
        }
    }
}

/// Structural check with a real URL parser, not string-prefix matching:
/// the URL must use the http scheme with no credentials, and name
/// `localhost` or a loopback IP literal. Anything the parser rejects, or
/// any host that is not loopback, fails closed.
fn advertised_url_is_loopback_http(url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };
    if parsed.scheme() != "http" || !parsed.has_authority() {
        return false;
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return false;
    }
    match parsed.host_str() {
        Some(host) => {
            let host = host
                .strip_prefix('[')
                .and_then(|host| host.strip_suffix(']'))
                .unwrap_or(host);
            host == "localhost" || host.parse::<IpAddr>().is_ok_and(|addr| addr.is_loopback())
        }
        None => false,
    }
}

impl From<AgentRpcTlsSettings> for MutualTlsIdentityFiles {
    fn from(settings: AgentRpcTlsSettings) -> Self {
        Self::new(
            settings.certificate_path,
            settings.private_key_path,
            settings.client_ca_path,
        )
    }
}

fn unexpected_server_completion(
    completion: std::result::Result<Result<ServerKind>, tokio::task::JoinError>,
) -> Error {
    match completed_server(completion) {
        Ok(kind) => Error::Message(format!("{kind} server stopped unexpectedly")),
        Err(error) => error,
    }
}

fn completed_server(
    completion: std::result::Result<Result<ServerKind>, tokio::task::JoinError>,
) -> Result<ServerKind> {
    completion.map_err(Error::wrap)?
}

#[cfg(test)]
mod tests {
    use super::{
        AgentRpcSettings, ControllerSettings, PublicHttpMode, PublicHttpSettings,
        parse_controller_settings, resolve_public_transport,
    };
    use loco_rs::{config::Config, environment::Environment};
    use std::{net::SocketAddr, path::Path};

    #[test]
    fn controller_rpc_settings_require_an_explicit_mtls_bundle() {
        let settings: ControllerSettings = serde_json::from_value(valid_settings()).unwrap();

        let AgentRpcSettings {
            enabled,
            binding,
            cluster_id,
            tls,
            agent_server_name_suffix: _,
        } = settings.agent_rpc;
        assert!(enabled);
        assert_eq!(binding.to_string(), "127.0.0.1:50051");
        assert_eq!(cluster_id, "cluster-1");
        assert_eq!(tls.client_ca_path.to_string_lossy(), "/certs/agent-ca.pem");
    }

    fn valid_settings() -> serde_json::Value {
        serde_json::json!({
            "agent_rpc": {
                "enabled": true,
                "binding": "127.0.0.1:50051",
                "cluster_id": "cluster-1",
                "tls": {
                    "certificate_path": "/certs/controller.pem",
                    "private_key_path": "/certs/controller-key.pem",
                    "client_ca_path": "/certs/agent-ca.pem"
                }
            },
            "public_http": { "mode": "plaintext" },
            "docker": { "host": "unix:///var/run/docker.sock" },
            "github_auth": { "enabled": false }
        })
    }

    #[test]
    fn controller_shutdown_grace_uses_shared_default_and_rejects_invalid_values() {
        let default = parse_controller_settings(valid_settings()).unwrap();
        assert_eq!(
            default.shutdown_grace.duration(),
            std::time::Duration::from_secs(20)
        );

        let mut settings = valid_settings();
        settings["shutdown_grace"] = serde_json::json!("250ms");
        let configured = parse_controller_settings(settings.clone()).unwrap();
        assert_eq!(
            configured.shutdown_grace.duration(),
            std::time::Duration::from_millis(250)
        );

        for value in [
            serde_json::json!("0s"),
            serde_json::json!("1h"),
            serde_json::json!(20),
        ] {
            settings["shutdown_grace"] = value;
            assert!(parse_controller_settings(settings.clone()).is_err());
        }
    }

    #[test]
    fn controller_rendered_shutdown_grace_child() {
        let Ok(expected) = std::env::var("KONTAINER_TEST_SHUTDOWN_GRACE_CHILD") else {
            return;
        };
        let config = Config::from_folder(&Environment::Development, Path::new("config")).unwrap();
        let settings = parse_controller_settings(config.settings.unwrap());
        if expected == "invalid" {
            assert!(settings.is_err());
        } else {
            assert_eq!(
                settings.unwrap().shutdown_grace.duration(),
                expected
                    .parse::<kontainer_lifecycle::ShutdownGrace>()
                    .unwrap()
                    .duration(),
            );
        }
    }

    #[test]
    fn controller_environment_override_changes_rendered_settings() {
        for (value, expected) in [("750ms", "750ms"), ("0s", "invalid")] {
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "server::tests::controller_rendered_shutdown_grace_child",
                ])
                .env("SHUTDOWN_GRACE", value)
                .env("KONTAINER_TEST_SHUTDOWN_GRACE_CHILD", expected)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "SHUTDOWN_GRACE={value} was not applied: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
        }
    }

    #[test]
    fn unknown_controller_owned_settings_are_rejected_at_every_level() {
        for pointer in [
            "",
            "/agent_rpc",
            "/agent_rpc/tls",
            "/public_http",
            "/docker",
            "/github_auth",
        ] {
            let mut settings = valid_settings();
            settings
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(
                    "unexpected_setting".to_owned(),
                    serde_json::json!("secret-value"),
                );
            let error = parse_controller_settings(settings).unwrap_err();
            assert!(!error.to_string().contains("secret-value"));
        }

        let mut settings = valid_settings();
        settings["public_http"] = serde_json::json!({
            "mode": "tls",
            "tls": {
                "certificate_path": "/certs/public.pem",
                "private_key_path": "/certs/public-key.pem",
                "unexpected_setting": "secret-value"
            }
        });
        let error = parse_controller_settings(settings).unwrap_err();
        assert!(!error.to_string().contains("secret-value"));
    }

    #[test]
    fn configured_cluster_ids_use_the_certificate_identity_rules() {
        for invalid in ["", ".", "..", "cluster/other", " secret-value "] {
            let mut settings = valid_settings();
            settings["agent_rpc"]["cluster_id"] = serde_json::json!(invalid);
            let error = serde_json::from_value::<ControllerSettings>(settings).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("cluster_id must be a canonical workload identity segment")
            );
            assert!(!error.to_string().contains("secret-value"));
        }
    }

    #[test]
    fn the_static_agent_client_setting_is_gone_and_the_suffix_is_validated() {
        let mut settings = valid_settings();
        settings["agent_rpc"]["client"] = serde_json::json!({
            "endpoint": "https://127.0.0.1:50052",
            "server_name": "node-1.agent.kontainer.test",
            "node_id": "node-1",
        });
        assert!(
            parse_controller_settings(settings).is_err(),
            "membership replaced the one configured agent"
        );

        let parsed = parse_controller_settings(valid_settings()).unwrap();
        assert_eq!(
            parsed.agent_rpc.agent_server_name_suffix,
            "agent.kontainer.internal"
        );
        let mut custom = valid_settings();
        custom["agent_rpc"]["agent_server_name_suffix"] = serde_json::json!("agents.example");
        assert_eq!(
            parse_controller_settings(custom)
                .unwrap()
                .agent_rpc
                .agent_server_name_suffix,
            "agents.example"
        );
        for invalid in [
            "",
            "under_score.example",
            "-bad.example",
            "a..b",
            "secret value",
        ] {
            let mut bad = valid_settings();
            bad["agent_rpc"]["agent_server_name_suffix"] = serde_json::json!(invalid);
            let error = parse_controller_settings(bad).unwrap_err();
            assert!(!error.to_string().contains("secret value"), "{invalid}");
        }
    }

    #[test]
    fn checked_in_non_production_configs_parse() {
        for environment in [Environment::Development, Environment::Test] {
            let config = Config::from_folder(&environment, Path::new("config")).unwrap();
            let settings: ControllerSettings =
                serde_json::from_value(config.settings.unwrap()).unwrap();

            // Development runs the agent RPC against dev/pki by default.
            assert_eq!(
                settings.agent_rpc.enabled,
                environment == Environment::Development
            );
            assert_eq!(settings.public_http.mode, PublicHttpMode::Plaintext);
            assert!(!settings.github_auth.enabled);
        }
    }

    #[test]
    fn docker_host_defaults_to_the_socket_and_tcp_is_development_only() {
        let settings = parse_controller_settings(valid_settings()).unwrap();
        assert_eq!(
            settings.docker.host,
            kontainer_engine::DockerHost::default()
        );

        let mut tcp = valid_settings();
        tcp["docker"] = serde_json::json!({ "host": "tcp://localhost:2375" });
        let settings = parse_controller_settings(tcp).unwrap();
        for environment in [Environment::Development, Environment::Test] {
            assert!(super::validate_docker_host(&settings.docker, &environment).is_ok());
        }
        let error =
            super::validate_docker_host(&settings.docker, &Environment::Production).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("tcp:// only in development or test")
        );

        for invalid in ["http://localhost:2375", "localhost:2375", ""] {
            let mut settings = valid_settings();
            settings["docker"] = serde_json::json!({ "host": invalid });
            assert!(parse_controller_settings(settings).is_err(), "{invalid}");
        }
    }

    #[test]
    fn github_auth_settings_require_secret_and_allowlist_when_enabled() {
        let mut missing_secret = valid_settings();
        missing_secret["github_auth"] = serde_json::json!({
            "enabled": true,
            "client_id": "client-1",
            "allowlisted_account_ids": "42"
        });
        let error = parse_controller_settings(missing_secret).unwrap_err();
        assert!(!error.to_string().contains("secret-value"));

        let mut bad_client = valid_settings();
        bad_client["github_auth"] = serde_json::json!({
            "enabled": true,
            "client_id": "not valid!",
            "client_secret_path": "/secrets/github",
            "allowlisted_account_ids": "42"
        });
        assert!(parse_controller_settings(bad_client).is_err());

        let mut empty_allowlist = valid_settings();
        empty_allowlist["github_auth"] = serde_json::json!({
            "enabled": true,
            "client_id": "client-1",
            "client_secret_path": "/secrets/github",
            "allowlisted_account_ids": ""
        });
        assert!(parse_controller_settings(empty_allowlist).is_err());

        let mut enabled = valid_settings();
        enabled["github_auth"] = serde_json::json!({
            "enabled": true,
            "client_id": "client-1",
            "client_secret_path": "/secrets/github",
            "allowlisted_account_ids": "42, 7"
        });
        let settings: ControllerSettings = serde_json::from_value(enabled.clone()).unwrap();
        assert!(settings.github_auth.enabled);
        assert!(parse_controller_settings(enabled).is_ok());

        // Disabled login ignores the remaining fields but still rejects
        // unknown keys and secret-bearing error text.
        let mut disabled = valid_settings();
        disabled["github_auth"] = serde_json::json!({ "enabled": false });
        assert!(parse_controller_settings(disabled).is_ok());
    }

    #[test]
    fn plaintext_mode_forbids_tls_paths_and_tls_mode_requires_them() {
        let mut plaintext_with_tls = valid_settings();
        plaintext_with_tls["public_http"]["tls"] = serde_json::json!({
            "certificate_path": "/certs/public.pem",
            "private_key_path": "/certs/public-key.pem"
        });
        let error = parse_controller_settings(plaintext_with_tls).unwrap_err();
        assert!(error.to_string().contains("forbids"));

        let mut tls_without_paths = valid_settings();
        tls_without_paths["public_http"] = serde_json::json!({ "mode": "tls" });
        let error = parse_controller_settings(tls_without_paths).unwrap_err();
        assert!(error.to_string().contains("requires"));

        let mut unknown_mode = valid_settings();
        unknown_mode["public_http"] = serde_json::json!({ "mode": "starttls" });
        assert!(parse_controller_settings(unknown_mode).is_err());

        let mut tls = valid_settings();
        tls["public_http"] = serde_json::json!({
            "mode": "tls",
            "tls": {
                "certificate_path": "/certs/public.pem",
                "private_key_path": "/certs/public-key.pem"
            }
        });
        let settings = parse_controller_settings(tls).unwrap();
        assert_eq!(settings.public_http.mode, PublicHttpMode::Tls);
        assert!(settings.public_http.tls.is_some());
    }

    #[test]
    fn unknown_public_http_settings_are_rejected() {
        let mut settings = valid_settings();
        settings["public_http"]
            .as_object_mut()
            .unwrap()
            .insert("unexpected_setting".to_owned(), serde_json::json!(true));
        assert!(parse_controller_settings(settings).is_err());
    }

    fn plaintext_settings() -> PublicHttpSettings {
        serde_json::from_value(serde_json::json!({ "mode": "plaintext" })).unwrap()
    }

    #[tokio::test]
    async fn plaintext_is_confined_to_development_and_test_loopback() {
        use std::net::{IpAddr, Ipv4Addr};

        let settings = plaintext_settings();
        let loopback = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 5150);
        for environment in [Environment::Development, Environment::Test] {
            for host in [
                "http://127.0.0.1:5150",
                "http://localhost:5150",
                "http://[::1]:5150",
            ] {
                assert!(
                    resolve_public_transport(&settings, &environment, loopback, host)
                        .await
                        .is_ok(),
                    "{environment:?} {host} must allow loopback plaintext"
                );
            }
        }

        for environment in [
            Environment::Production,
            Environment::Any("staging".to_owned()),
        ] {
            let error = resolve_public_transport(
                &settings,
                &environment,
                loopback,
                "http://127.0.0.1:5150",
            )
            .await
            .unwrap_err();
            assert!(error.to_string().contains("plaintext public HTTP"));
        }

        let public = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 5150);
        let error = resolve_public_transport(
            &settings,
            &Environment::Development,
            public,
            "http://127.0.0.1:5150",
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("must bind loopback"));

        for host in [
            "http://192.168.1.10:5150",
            "https://127.0.0.1:5150",
            "http://example.com:5150",
            "not-a-url",
            "http://user:pass@127.0.0.1:5150/",
            "http://evil@localhost/",
            "http://evil\\@localhost/",
            "http://[::1].evil/",
            "http://127.0.0.1:99999/",
            "http://localhost.:5150/",
            "http://::1:5150/",
        ] {
            let error =
                resolve_public_transport(&settings, &Environment::Development, loopback, host)
                    .await
                    .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("must advertise a loopback http URL"),
                "{host} must be rejected"
            );
        }

        // URL schemes are case-insensitive; the parser normalizes them.
        assert!(
            resolve_public_transport(
                &settings,
                &Environment::Development,
                loopback,
                "HTTP://127.0.0.1:5150/",
            )
            .await
            .is_ok(),
            "uppercase scheme with a loopback host must be accepted"
        );
    }

    #[tokio::test]
    async fn tls_identities_fail_closed_before_admission() {
        let directory = tempfile::tempdir().unwrap();
        let missing = |name: &str| directory.path().join(name);

        let missing_settings = tls_settings(missing("public.pem"), missing("public-key.pem"));
        let error = resolve_public_transport(
            &missing_settings,
            &Environment::Production,
            loopback(),
            "https://controller.example:5150",
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("invalid public TLS identity"));

        let malformed = directory.path().join("malformed.pem");
        std::fs::write(&malformed, "secret-canary-invalid-pem").unwrap();
        let malformed_settings = tls_settings(malformed.clone(), malformed);
        let error = resolve_public_transport(
            &malformed_settings,
            &Environment::Production,
            loopback(),
            "https://controller.example:5150",
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("invalid public TLS identity"));
        assert!(!error.to_string().contains("secret-canary"));

        let first = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
        let second = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
        let cert_path = directory.path().join("public.pem");
        let key_path = directory.path().join("public-key.pem");
        std::fs::write(&cert_path, first.cert.pem()).unwrap();
        std::fs::write(&key_path, second.signing_key.serialize_pem()).unwrap();
        let mismatched_settings = tls_settings(cert_path, key_path);
        let error = resolve_public_transport(
            &mismatched_settings,
            &Environment::Production,
            loopback(),
            "https://controller.example:5150",
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("invalid public TLS identity"));
    }

    fn tls_settings(
        certificate: std::path::PathBuf,
        key: std::path::PathBuf,
    ) -> PublicHttpSettings {
        serde_json::from_value(serde_json::json!({
            "mode": "tls",
            "tls": {
                "certificate_path": certificate,
                "private_key_path": key
            }
        }))
        .unwrap()
    }

    fn loopback() -> SocketAddr {
        SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 5150)
    }
}
