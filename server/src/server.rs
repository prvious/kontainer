use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use axum::{
    Router,
    extract::Request,
    middleware::{self, Next},
    response::Response,
};
use hyper::server::conn::http1;
use hyper_util::{rt::TokioIo, service::TowerToHyperService};
use kontainer_rpc_auth::{MutualTlsIdentityFiles, valid_identity_segment};
use loco_rs::{
    Error, Result,
    app::AppContext,
    boot::{ServeParams, shutdown_signal},
    environment::Environment,
};
use serde::Deserialize;
use tokio::{net::TcpListener, task::JoinSet};
use tokio_util::sync::CancellationToken;

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
    github_auth: GitHubAuthSettings,
}

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
        Some(BoundAgentRpcServer {
            listener: TcpListener::bind(settings.agent_rpc.binding).await?,
            cluster_id: settings.agent_rpc.cluster_id,
            tls_files: settings.agent_rpc.tls.into(),
        })
    } else {
        None
    };
    let cancellation = ctx
        .shared_store
        .get::<CancellationToken>()
        .ok_or_else(|| Error::string("controller lifecycle is not initialized"))?;
    let servers = supervise_controller_servers(
        router,
        BoundHttpServer {
            listener: http_listener,
            transport,
        },
        agent_rpc,
        cancellation.clone(),
    );
    tokio::pin!(servers);

    tokio::select! {
        result = &mut servers => result,
        _ = shutdown_signal() => {
            cancellation.cancel();
            servers.await
        }
    }
}

pub struct BoundAgentRpcServer {
    pub listener: TcpListener,
    pub cluster_id: String,
    pub tls_files: MutualTlsIdentityFiles,
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

/// Serve the public router over TLS: accept TCP connections, handshake each
/// one concurrently, and serve HTTP/1 per connection. Handshake and protocol
/// failures are connection-local; only listener failures fail the server.
/// Cancellation stops admission immediately, cancels pending handshakes, and
/// gracefully drains admitted connections before every task is joined.
async fn serve_tls(
    listener: TcpListener,
    router: Router,
    config: Arc<rustls::ServerConfig>,
    cancellation: CancellationToken,
) -> Result<ServerKind> {
    let acceptor = tokio_rustls::TlsAcceptor::from(config);
    let mut connections = JoinSet::new();
    let mut failure: Option<Error> = None;

    loop {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => break,
            completion = connections.join_next(), if !connections.is_empty() => {
                match completion {
                    Some(Ok(())) => {}
                    Some(Err(error)) => {
                        failure = Some(Error::wrap(error));
                        cancellation.cancel();
                        break;
                    }
                    None => {}
                }
            }
            accepted = listener.accept() => {
                let (socket, peer) = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        failure = Some(Error::wrap(error));
                        cancellation.cancel();
                        break;
                    }
                };
                let router = router.clone();
                let acceptor = acceptor.clone();
                let connection_cancel = cancellation.clone();
                connections.spawn(async move {
                    serve_tls_connection(router, acceptor, socket, peer, connection_cancel).await;
                });
            }
        }
    }

    while let Some(completion) = connections.join_next().await {
        if failure.is_none()
            && let Err(error) = completion
        {
            failure = Some(Error::wrap(error));
        }
    }

    failure.map_or(Ok(ServerKind::Https), Err)
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
    cancellation: CancellationToken,
) -> Result<()> {
    let agent_rpc = agent_rpc
        .map(|agent_rpc| {
            serve_controller_rpc(
                agent_rpc.listener,
                agent_rpc.cluster_id,
                agent_rpc.tls_files,
                cancellation.clone(),
            )
        })
        .transpose()
        .map_err(|error| {
            cancellation.cancel();
            Error::Message(format!("agent RPC server failed: {error}"))
        })?;
    let mut servers = JoinSet::new();

    if cancellation.is_cancelled() {
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

    let http_cancellation = cancellation.clone();
    match http.transport {
        HttpTransport::Plaintext => {
            servers.spawn(async move {
                axum::serve(
                    http.listener,
                    router.into_make_service_with_connect_info::<SocketAddr>(),
                )
                .with_graceful_shutdown(http_cancellation.cancelled_owned())
                .await
                .map_err(Error::wrap)?;

                Ok(ServerKind::Http)
            });
        }
        HttpTransport::Tls(config) => {
            let tls_cancellation = cancellation.clone();
            servers.spawn(serve_tls(http.listener, router, config, tls_cancellation));
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

    let first_completion = tokio::select! {
        biased;
        _ = cancellation.cancelled() => None,
        completion = servers.join_next() => completion,
    };

    let mut failure = first_completion.map(unexpected_server_completion);
    cancellation.cancel();

    while let Some(completion) = servers.join_next().await {
        if failure.is_none()
            && let Err(error) = completed_server(completion)
        {
            failure = Some(error);
        }
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
                "invalid controller settings: check agent_rpc, public_http, and github_auth keys, types, and cluster_id",
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
pub fn init_shared_auth_state(ctx: &AppContext) -> Result<()> {
    use std::sync::Arc;

    let settings = controller_settings(ctx)?;
    ctx.shared_store
        .insert(ControllerClusterId(settings.agent_rpc.cluster_id.clone()));

    let service = if settings.github_auth.enabled {
        let secret_path = settings
            .github_auth
            .client_secret_path
            .clone()
            .ok_or_else(|| Error::string("github_auth client secret is unavailable"))?;
        let raw = std::fs::read_to_string(&secret_path)
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
            "github_auth": { "enabled": false }
        })
    }

    #[test]
    fn unknown_controller_owned_settings_are_rejected_at_every_level() {
        for pointer in [
            "",
            "/agent_rpc",
            "/agent_rpc/tls",
            "/public_http",
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
    fn checked_in_non_production_configs_parse() {
        for environment in [Environment::Development, Environment::Test] {
            let config = Config::from_folder(&environment, Path::new("config")).unwrap();
            let settings: ControllerSettings =
                serde_json::from_value(config.settings.unwrap()).unwrap();

            assert!(!settings.agent_rpc.enabled);
            assert_eq!(settings.public_http.mode, PublicHttpMode::Plaintext);
            assert!(!settings.github_auth.enabled);
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
