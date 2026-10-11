//! Public HTTP transport tests for issue #77.
//!
//! A configured TLS identity fails closed before any handler runs; without
//! one the controller serves plaintext behind the operator's reverse proxy,
//! which terminates TLS (ADR 0046). These tests drive the real
//! `server::serve` entry point and the real supervisor over real TCP/TLS:
//! no handshake outcome is mocked, and handler counters prove whether
//! traffic reached application code.

use std::{
    future::pending,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{Router, routing::get};
use kontainer_controller::{
    app::App,
    controllers::health,
    server::{
        BoundHttpServer, ControllerShutdown, HttpTransport, PublicTlsSettings,
        load_public_tls_config, serve,
    },
};
use kontainer_lifecycle::ShutdownGrace;
use loco_rs::{app::Hooks, config::Config, environment::Environment};
use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose, SanType,
};
use sea_orm::DatabaseConnection;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Semaphore,
    time::timeout,
};
use tokio_rustls::{TlsConnector, client::TlsStream};
use tokio_util::sync::CancellationToken;

const BUDGET: Duration = Duration::from_secs(5);
const TEST_DATABASE_URL: &str = "KONTAINER_TEST_DATABASE_URL";
const REQUIRE_POSTGRES_TESTS: &str = "KONTAINER_REQUIRE_POSTGRES_TESTS";

/// Purpose-built public PKI: a CA plus a server certificate trusted for both
/// the `localhost` DNS name and the `127.0.0.1` IP. Deliberately separate
/// from the mTLS workload-identity fixtures in `support::pki`.
struct PublicTestPki {
    _directory: tempfile::TempDir,
    ca_pem: String,
    server_cert: PathBuf,
    server_key: PathBuf,
    other_ca_pem: String,
    chained_cert: PathBuf,
    chained_key: PathBuf,
    unrelated_key: PathBuf,
}

impl PublicTestPki {
    fn generate() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let (ca_pem, ca_issuer) = authority(directory.path(), "ca", "Public TLS test CA");
        let (other_ca_pem, _) = authority(directory.path(), "other-ca", "Unrelated test CA");

        let mut server = CertificateParams::new(vec!["localhost".to_owned()]).unwrap();
        server
            .distinguished_name
            .push(DnType::CommonName, "controller");
        server
            .subject_alt_names
            .push(SanType::IpAddress("127.0.0.1".parse().unwrap()));
        server.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        server.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let server_key = KeyPair::generate().unwrap();
        let server_cert = server.signed_by(&server_key, &ca_issuer).unwrap();
        let server_cert_path = directory.path().join("server.pem");
        let server_key_path = directory.path().join("server-key.pem");
        std::fs::write(&server_cert_path, server_cert.pem()).unwrap();
        std::fs::write(&server_key_path, server_key.serialize_pem()).unwrap();

        // Chained bundle: a leaf issued by an intermediate, served as
        // leaf-plus-intermediate exactly like production fullchain files.
        let mut intermediate_params = CertificateParams::new(Vec::new()).unwrap();
        intermediate_params
            .distinguished_name
            .push(DnType::CommonName, "test intermediate");
        intermediate_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let intermediate_key = KeyPair::generate().unwrap();
        let intermediate_cert = intermediate_params
            .signed_by(&intermediate_key, &ca_issuer)
            .unwrap();
        let intermediate_issuer = Issuer::new(intermediate_params, intermediate_key);
        let mut chained_params = CertificateParams::new(vec!["localhost".to_owned()]).unwrap();
        chained_params
            .distinguished_name
            .push(DnType::CommonName, "chained");
        chained_params
            .subject_alt_names
            .push(SanType::IpAddress("127.0.0.1".parse().unwrap()));
        chained_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        chained_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let chained_key = KeyPair::generate().unwrap();
        let chained_leaf = chained_params
            .signed_by(&chained_key, &intermediate_issuer)
            .unwrap();
        let chained_cert_path = directory.path().join("chained.pem");
        let chained_key_path = directory.path().join("chained-key.pem");
        std::fs::write(
            &chained_cert_path,
            format!("{}{}", chained_leaf.pem(), intermediate_cert.pem()),
        )
        .unwrap();
        std::fs::write(&chained_key_path, chained_key.serialize_pem()).unwrap();

        let unrelated_key = KeyPair::generate().unwrap();
        let unrelated_key_path = directory.path().join("unrelated-key.pem");
        std::fs::write(&unrelated_key_path, unrelated_key.serialize_pem()).unwrap();

        Self {
            _directory: directory,
            ca_pem,
            server_cert: server_cert_path,
            server_key: server_key_path,
            other_ca_pem,
            chained_cert: chained_cert_path,
            chained_key: chained_key_path,
            unrelated_key: unrelated_key_path,
        }
    }

    fn identity(&self) -> PublicTlsSettings {
        PublicTlsSettings {
            certificate_path: self.server_cert.clone(),
            private_key_path: self.server_key.clone(),
        }
    }
}

fn authority(
    directory: &Path,
    name: &str,
    common_name: &str,
) -> (String, Issuer<'static, KeyPair>) {
    let mut parameters = CertificateParams::new(Vec::new()).unwrap();
    parameters
        .distinguished_name
        .push(DnType::CommonName, common_name);
    parameters.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let key_pair = KeyPair::generate().unwrap();
    let certificate = parameters.self_signed(&key_pair).unwrap();
    let path = directory.join(format!("{name}.pem"));
    std::fs::write(&path, certificate.pem()).unwrap();
    (certificate.pem(), Issuer::new(parameters, key_pair))
}

/// Explicit ring provider on both ends: provider auto-detect panics and must
/// stay out of every construction path, production and test alike.
fn tls_connector(ca_pem: &str) -> TlsConnector {
    use rustls::pki_types::pem::PemObject;

    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from_pem_slice(ca_pem.as_bytes()).unwrap())
        .unwrap();
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(rustls::ALL_VERSIONS)
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    TlsConnector::from(Arc::new(config))
}

async fn tls_stream(
    address: SocketAddr,
    connector: &TlsConnector,
    name: &str,
) -> std::io::Result<TlsStream<TcpStream>> {
    let tcp = timeout(BUDGET, TcpStream::connect(address))
        .await
        .map_err(|_| std::io::Error::other("TCP connect timed out"))??;
    let name =
        rustls::pki_types::ServerName::try_from(name.to_owned()).map_err(std::io::Error::other)?;
    timeout(BUDGET, connector.connect(name, tcp))
        .await
        .map_err(|_| std::io::Error::other("TLS handshake timed out"))?
}

async fn raw_get(stream: &mut TlsStream<TcpStream>, path: &str) -> std::io::Result<String> {
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    timeout(BUDGET, stream.write_all(request.as_bytes()))
        .await
        .map_err(|_| std::io::Error::other("request write timed out"))??;
    let mut body = Vec::new();
    timeout(BUDGET, stream.read_to_end(&mut body))
        .await
        .map_err(|_| std::io::Error::other("response read timed out"))??;
    Ok(String::from_utf8_lossy(&body).into_owned().to_lowercase())
}

fn counter_router(calls: &Arc<AtomicUsize>) -> Router {
    let handler_calls = calls.clone();
    Router::new().route(
        "/application",
        get(move || {
            let handler_calls = handler_calls.clone();
            async move {
                handler_calls.fetch_add(1, Ordering::SeqCst);
                "application"
            }
        }),
    )
}

async fn start_tls_supervisor(
    router: Router,
    pki: &PublicTestPki,
) -> (SocketAddr, CancellationToken, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let config = load_public_tls_config(&pki.identity()).unwrap();
    let cancellation = CancellationToken::new();
    let task_cancellation = cancellation.clone();
    let server = tokio::spawn(async move {
        supervise(router, listener, config, task_cancellation).await;
    });
    (address, cancellation, server)
}

async fn supervise(
    router: Router,
    listener: TcpListener,
    config: Arc<rustls::ServerConfig>,
    cancellation: CancellationToken,
) {
    kontainer_controller::server::supervise_controller_servers(
        router,
        BoundHttpServer {
            listener,
            transport: HttpTransport::Tls(config),
        },
        None,
        cancellation,
    )
    .await
    .unwrap();
}

async fn join_server(server: tokio::task::JoinHandle<()>, cancellation: &CancellationToken) {
    cancellation.cancel();
    timeout(BUDGET, server)
        .await
        .expect("TLS supervisor did not join after cancellation")
        .unwrap();
}

#[tokio::test]
async fn tls_deadline_drops_a_stuck_handler() {
    struct DropCount(Arc<AtomicUsize>);
    impl Drop for DropCount {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let pki = PublicTestPki::generate();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let config = load_public_tls_config(&pki.identity()).unwrap();
    let entered = Arc::new(Semaphore::new(0));
    let dropped = Arc::new(AtomicUsize::new(0));
    let handler_entered = entered.clone();
    let handler_dropped = dropped.clone();
    let router = Router::new().route(
        "/stuck",
        get(move || {
            let entered = handler_entered.clone();
            let dropped = handler_dropped.clone();
            async move {
                let _guard = DropCount(dropped);
                entered.add_permits(1);
                pending::<&'static str>().await
            }
        }),
    );
    let shutdown = ControllerShutdown::new("100ms".parse::<ShutdownGrace>().unwrap());
    let task_shutdown = shutdown.clone();
    let task = tokio::spawn(async move {
        kontainer_controller::server::supervise_controller_servers(
            router,
            BoundHttpServer {
                listener,
                transport: HttpTransport::Tls(config),
            },
            None,
            task_shutdown,
        )
        .await
    });
    let connector = tls_connector(&pki.ca_pem);
    let mut stream = tls_stream(address, &connector, "localhost").await.unwrap();
    stream
        .write_all(b"GET /stuck HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    entered.acquire().await.unwrap().forget();
    shutdown.begin_drain();

    let result = timeout(BUDGET, task).await.unwrap().unwrap();
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("deadline exceeded")
    );
    assert!(shutdown.force().is_cancelled());
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert!(TcpStream::connect(address).await.is_err());
}

#[tokio::test]
async fn trusted_client_with_expected_dns_reaches_exactly_one_handler() {
    let pki = PublicTestPki::generate();
    let calls = Arc::new(AtomicUsize::new(0));
    let (address, cancellation, server) = start_tls_supervisor(counter_router(&calls), &pki).await;

    let connector = tls_connector(&pki.ca_pem);
    let mut stream = tls_stream(address, &connector, "localhost")
        .await
        .expect("trusted client with the expected DNS name must complete TLS");
    let response = raw_get(&mut stream, "/application")
        .await
        .expect("trusted client must receive an HTTP response");

    assert!(
        response.contains("200 ok"),
        "unexpected response: {response}"
    );
    assert!(response.contains("application"));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    join_server(server, &cancellation).await;
}

#[tokio::test]
async fn unknown_certificate_authority_fails_before_any_handler() {
    let pki = PublicTestPki::generate();
    let calls = Arc::new(AtomicUsize::new(0));
    let (address, cancellation, server) = start_tls_supervisor(counter_router(&calls), &pki).await;

    // Correct DNS name, but a CA the server identity was never issued from.
    let connector = tls_connector(&pki.other_ca_pem);
    let handshake = tls_stream(address, &connector, "localhost").await;

    assert!(
        handshake.is_err(),
        "unknown CA must fail the TLS handshake, not the HTTP layer"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "a rejected handshake must never reach a handler"
    );
    join_server(server, &cancellation).await;
}

#[tokio::test]
async fn wrong_dns_name_fails_before_any_handler() {
    let pki = PublicTestPki::generate();
    let calls = Arc::new(AtomicUsize::new(0));
    let (address, cancellation, server) = start_tls_supervisor(counter_router(&calls), &pki).await;

    // Issuing CA is trusted, but the name is absent from the certificate.
    // This is TLS name validation, not an HTTP Host check.
    let connector = tls_connector(&pki.ca_pem);
    let handshake = tls_stream(address, &connector, "wrong.invalid").await;

    assert!(
        handshake.is_err(),
        "wrong DNS name must fail the TLS handshake, not the HTTP layer"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "a rejected handshake must never reach a handler"
    );
    join_server(server, &cancellation).await;
}

#[tokio::test]
async fn chained_bundle_serves_the_intermediate_to_root_clients() {
    let pki = PublicTestPki::generate();
    let calls = Arc::new(AtomicUsize::new(0));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let chained = PublicTlsSettings {
        certificate_path: pki.chained_cert.clone(),
        private_key_path: pki.chained_key.clone(),
    };
    let config = load_public_tls_config(&chained).unwrap();
    let router = counter_router(&calls);
    let cancellation = CancellationToken::new();
    let task_cancellation = cancellation.clone();
    let server = tokio::spawn(async move {
        supervise(router, listener, config, task_cancellation).await;
    });

    // The client trusts only the root: the handshake succeeds only if the
    // server transmits the intermediate alongside the leaf.
    let connector = tls_connector(&pki.ca_pem);
    let mut stream = tls_stream(address, &connector, "localhost")
        .await
        .expect("chained bundle must validate against the root");
    let response = raw_get(&mut stream, "/application")
        .await
        .expect("read failed");
    assert!(
        response.contains("200 ok"),
        "unexpected response: {response}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    join_server(server, &cancellation).await;
}

#[tokio::test]
async fn plaintext_bytes_to_the_tls_listener_never_reach_a_handler() {
    let pki = PublicTestPki::generate();
    let calls = Arc::new(AtomicUsize::new(0));
    let (address, cancellation, server) = start_tls_supervisor(counter_router(&calls), &pki).await;

    let mut plain = timeout(BUDGET, TcpStream::connect(address))
        .await
        .expect("TCP connect timed out")
        .expect("TCP connect failed");
    timeout(
        BUDGET,
        plain.write_all(
            b"GET /application HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        ),
    )
    .await
    .expect("write timed out")
    .expect("write failed");
    let mut bytes = Vec::new();
    let read = timeout(BUDGET, plain.read_to_end(&mut bytes)).await;

    let text = String::from_utf8_lossy(&bytes).into_owned().to_lowercase();
    assert!(
        read.is_err() || !text.starts_with("http/"),
        "plaintext bytes must never yield an HTTP response, got: {text}"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "plaintext bytes must never reach a handler"
    );
    join_server(server, &cancellation).await;
}

async fn serve_context(
    environment: Environment,
    settings: serde_json::Value,
    host: &str,
) -> loco_rs::app::AppContext {
    let mut config = Config::from_folder(&Environment::Test, Path::new("config")).unwrap();
    config.settings = Some(settings);
    config.server.host = host.to_owned();
    let context =
        loco_rs::app::AppContext::builder(environment, DatabaseConnection::default(), config)
            .build();
    App::after_context(context).await.unwrap()
}

fn test_settings() -> serde_json::Value {
    let config = Config::from_folder(&Environment::Test, Path::new("config")).unwrap();
    config.settings.unwrap()
}

fn serve_params(binding: &str) -> loco_rs::boot::ServeParams {
    loco_rs::boot::ServeParams {
        port: 0,
        binding: binding.to_owned(),
    }
}

/// Behind a reverse proxy the controller serves plaintext on every interface (ADR 0046); the
/// proxy terminates TLS. Real TCP, the real supervisor, and a handler counter prove the request
/// reached application code.
#[tokio::test]
async fn plaintext_on_every_interface_reaches_the_handler() {
    let calls = Arc::new(AtomicUsize::new(0));
    let listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let cancellation = CancellationToken::new();
    let task_cancellation = cancellation.clone();
    let router = counter_router(&calls);
    let server = tokio::spawn(async move {
        kontainer_controller::server::supervise_controller_servers(
            router,
            BoundHttpServer {
                listener,
                transport: HttpTransport::Plaintext,
            },
            None,
            task_cancellation,
        )
        .await
        .unwrap();
    });

    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    stream
        .write_all(
            b"GET /application HTTP/1.1\r\nHost: kontainer.example\r\nConnection: close\r\n\r\n",
        )
        .await
        .unwrap();
    let mut response = String::new();
    timeout(BUDGET, stream.read_to_string(&mut response))
        .await
        .unwrap()
        .unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.ends_with("application"), "{response}");
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    join_server(server, &cancellation).await;
}

fn tls_test_settings(cert: &Path, key: &Path) -> serde_json::Value {
    let mut settings = test_settings();
    settings["public_http"] = serde_json::json!({
        "mode": "tls",
        "tls": {
            "certificate_path": cert,
            "private_key_path": key
        }
    });
    settings
}

#[tokio::test]
async fn invalid_tls_identities_fail_the_serve_startup_path() {
    let pki = PublicTestPki::generate();
    let missing = pki.server_cert.with_file_name("missing.pem");
    let malformed_path = pki.server_cert.with_file_name("malformed.pem");
    std::fs::write(&malformed_path, "secret-canary-invalid-pem").unwrap();

    let cases: Vec<(&str, PathBuf, PathBuf)> = vec![
        (
            "missing certificate",
            missing.clone(),
            pki.server_key.clone(),
        ),
        ("missing key", pki.server_cert.clone(), missing.clone()),
        (
            "malformed certificate",
            malformed_path.clone(),
            pki.server_key.clone(),
        ),
        (
            "malformed key",
            pki.server_cert.clone(),
            malformed_path.clone(),
        ),
        (
            "mismatched identity",
            pki.server_cert.clone(),
            pki.unrelated_key.clone(),
        ),
    ];
    for (name, cert, key) in cases {
        let calls = Arc::new(AtomicUsize::new(0));
        let context = serve_context(
            Environment::Production,
            tls_test_settings(&cert, &key),
            "https://controller.example:5150",
        )
        .await;
        let error = serve(counter_router(&calls), &context, &serve_params("127.0.0.1"))
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("invalid public TLS identity"),
            "{name} unexpected error: {error}"
        );
        assert!(
            !error.to_string().contains("secret-canary"),
            "{name} leaked file contents"
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "{name} must admit zero handlers"
        );
    }
}

fn require_test_database() -> Option<String> {
    match std::env::var(TEST_DATABASE_URL) {
        Ok(url) => Some(url),
        Err(_) => {
            if std::env::var(REQUIRE_POSTGRES_TESTS).as_deref() == Ok("1") {
                panic!("{TEST_DATABASE_URL} is required when {REQUIRE_POSTGRES_TESTS}=1");
            }
            eprintln!("skipping public TLS probe test: {TEST_DATABASE_URL} is not set");
            None
        }
    }
}

#[tokio::test]
async fn health_probes_keep_their_contract_over_tls_through_drain() {
    let Some(database_url) = require_test_database() else {
        return;
    };
    // Ping only: this test mutates no schema and creates no database.
    let database = sea_orm::Database::connect(&database_url).await.unwrap();
    let config = Config::from_folder(&Environment::Test, Path::new("config")).unwrap();
    let context = loco_rs::app::AppContext::builder(Environment::Test, database, config).build();
    let context = App::after_context(context).await.unwrap();
    let cancellation = context
        .shared_store
        .get::<CancellationToken>()
        .expect("controller lifecycle is not initialized");

    let calls = Arc::new(AtomicUsize::new(0));
    let handler_calls = calls.clone();
    let router = Router::new()
        .route("/health/live", get(health::live))
        .route("/health/startup", get(health::startup))
        .route("/health/ready", get(health::ready))
        .route(
            "/application",
            get(move || {
                let handler_calls = handler_calls.clone();
                async move {
                    handler_calls.fetch_add(1, Ordering::SeqCst);
                    "application"
                }
            }),
        )
        .with_state(context);

    let pki = PublicTestPki::generate();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let config = load_public_tls_config(&pki.identity()).unwrap();
    let task_cancellation = cancellation.clone();
    let server = tokio::spawn(async move {
        kontainer_controller::server::supervise_controller_servers(
            router,
            BoundHttpServer {
                listener,
                transport: HttpTransport::Tls(config),
            },
            None,
            task_cancellation,
        )
        .await
        .unwrap();
    });

    let connector = tls_connector(&pki.ca_pem);
    for (path, expected_status, expected_body) in [
        ("/health/live", "200 ok", "\"ok\":true"),
        ("/health/startup", "200 ok", "\"ok\":true"),
        ("/health/ready", "200 ok", "\"ok\":true"),
        ("/application", "200 ok", "application"),
    ] {
        let mut stream = tls_stream(address, &connector, "localhost")
            .await
            .expect("pre-drain TLS must succeed");
        let response = raw_get(&mut stream, path).await.expect("read failed");
        assert!(
            response.contains(expected_status),
            "{path} unexpected response: {response}"
        );
        assert!(
            response.contains(expected_body),
            "{path} unexpected response: {response}"
        );
        if path.starts_with("/health") {
            assert!(
                response.contains("cache-control: no-store"),
                "{path} must stay uncacheable over TLS: {response}"
            );
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    // Drain: the supervisor joins promptly, and the closed listener refuses
    // new connections. Middleware-level rejection during the drain window is
    // covered deterministically by the transport-neutral unit test in
    // `controllers::health`.
    join_server(server, &cancellation).await;
    assert!(
        TcpStream::connect(address).await.is_err(),
        "a drained listener must refuse new connections"
    );
}
