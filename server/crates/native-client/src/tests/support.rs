//! Loopback HTTP/TLS fixture servers and PKI for transport-level tests.
//!
//! The servers speak raw HTTP/1.1 over real TCP and TLS so that no handshake,
//! redirect, reset, or timeout behavior is mocked. Every fixture records each
//! accepted connection and the exact request bytes it received, which lets
//! tests prove both what the client sent and what it never sent.

use std::{
    io,
    net::SocketAddr,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose, SanType,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::Notify,
    task::JoinHandle,
};
use tokio_rustls::{TlsAcceptor, server::TlsStream};

use crate::client::{NativeClient, NativeClientConfig, TrustPolicy};

pub(crate) const TEST_TIMEOUT: Duration = Duration::from_secs(5);
/// Deadline short enough to keep timeout tests quick.
pub(crate) const SHORT_DEADLINE: Duration = Duration::from_millis(250);

/// Await a fixture future under the shared test budget.
pub(crate) async fn wait<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(TEST_TIMEOUT, future)
        .await
        .expect("fixture did not complete in time")
}

/// What the fixture server does with one accepted connection.
#[derive(Debug, Clone)]
pub(crate) enum Behavior {
    /// Read the request, then answer with a fixed body.
    Respond { status: u16, body: String },
    /// Read the request, then answer with extra response headers.
    RespondWithHeaders {
        status: u16,
        body: String,
        headers: Vec<(&'static str, String)>,
    },
    /// Read the request, then answer with chunked transfer encoding.
    Chunked { status: u16, body: String },
    /// Read the request, then declare a body that is never completed.
    Truncated {
        declared_length: usize,
        body: String,
    },
    /// Read the request, then drop the connection with a TCP reset.
    Reset,
    /// Read the request, then hold the connection open until the peer closes.
    Stall,
}

pub(crate) fn respond(status: u16, body: &str) -> Behavior {
    Behavior::Respond {
        status,
        body: body.to_owned(),
    }
}

/// One request exactly as the fixture received it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturedRequest {
    pub(crate) method: String,
    pub(crate) target: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: String,
}

impl CapturedRequest {
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Default)]
struct ServerState {
    connections: AtomicUsize,
    requests: Mutex<Vec<CapturedRequest>>,
    request_seen: Notify,
    peer_closed: AtomicBool,
    peer_closed_signal: Notify,
}

/// A scripted loopback server: one scripted behavior per accepted connection.
pub(crate) struct TestServer {
    scheme: &'static str,
    address: SocketAddr,
    state: Arc<ServerState>,
    task: JoinHandle<()>,
}

impl TestServer {
    pub(crate) async fn plain(script: Vec<Behavior>) -> Self {
        Self::start("http", None, script).await
    }

    pub(crate) async fn tls(config: Arc<rustls::ServerConfig>, script: Vec<Behavior>) -> Self {
        Self::start("https", Some(config), script).await
    }

    async fn start(
        scheme: &'static str,
        tls: Option<Arc<rustls::ServerConfig>>,
        script: Vec<Behavior>,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("fixture listener binds");
        let address = listener.local_addr().expect("fixture address");
        let state = Arc::new(ServerState::default());
        let server_state = state.clone();
        assert!(!script.is_empty(), "fixture script must not be empty");
        let task = tokio::spawn(async move {
            let mut index = 0usize;
            loop {
                // The last entry repeats: tests assert exact connection
                // counts, so a client that connects more often than scripted
                // still gets deterministic behavior.
                let behavior = script[index.min(script.len() - 1)].clone();
                index += 1;
                let Ok((tcp, _)) = listener.accept().await else {
                    return;
                };
                server_state.connections.fetch_add(1, Ordering::SeqCst);
                let stream = match &tls {
                    None => Some(TestStream::Plain(tcp)),
                    Some(config) => TlsAcceptor::from(config.clone())
                        .accept(tcp)
                        .await
                        .ok()
                        .map(|tls| TestStream::Tls(Box::new(tls))),
                };
                if let Some(stream) = stream {
                    serve(stream, behavior, &server_state).await;
                }
            }
        });
        Self {
            scheme,
            address,
            state,
            task,
        }
    }

    /// Origin string for a client, for example `http://127.0.0.1:53123`.
    pub(crate) fn origin(&self) -> String {
        format!("{}://{}", self.scheme, self.address)
    }

    pub(crate) fn connection_count(&self) -> usize {
        self.state.connections.load(Ordering::SeqCst)
    }

    pub(crate) fn requests(&self) -> Vec<CapturedRequest> {
        self.state
            .requests
            .lock()
            .expect("fixture request lock")
            .clone()
    }

    pub(crate) fn request_count(&self) -> usize {
        self.state
            .requests
            .lock()
            .expect("fixture request lock")
            .len()
    }

    /// Wait until at least one request has been fully received.
    pub(crate) async fn wait_for_request(&self) {
        loop {
            if self.request_count() > 0 {
                return;
            }
            self.state.request_seen.notified().await;
        }
    }

    /// Wait until a stalled connection observed the client closing it.
    pub(crate) async fn wait_for_peer_close(&self) {
        loop {
            if self.state.peer_closed.load(Ordering::SeqCst) {
                return;
            }
            self.state.peer_closed_signal.notified().await;
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve(mut stream: TestStream, behavior: Behavior, state: &ServerState) {
    let Some(request) = read_request(&mut stream).await else {
        return;
    };
    state
        .requests
        .lock()
        .expect("fixture request lock")
        .push(request);
    state.request_seen.notify_one();
    match behavior {
        Behavior::Reset => {
            // SO_LINGER 0 turns close into a reset, which cannot be confused
            // with a clean end of a fixture response.
            if let TestStream::Plain(tcp) = &stream {
                let _ = socket2::SockRef::from(tcp).set_linger(Some(Duration::ZERO));
            }
        }
        Behavior::Stall => {
            let mut buffer = [0u8; 256];
            loop {
                match stream.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
            state.peer_closed.store(true, Ordering::SeqCst);
            state.peer_closed_signal.notify_one();
        }
        Behavior::Respond { status, body } => {
            let _ = stream
                .write_all(response_with_headers(status, &body, &[]).as_bytes())
                .await;
            let _ = stream.shutdown().await;
        }
        Behavior::RespondWithHeaders {
            status,
            body,
            headers,
        } => {
            let _ = stream
                .write_all(response_with_headers(status, &body, &headers).as_bytes())
                .await;
            let _ = stream.shutdown().await;
        }
        Behavior::Chunked { status, body } => {
            let _ = stream
                .write_all(chunked_response(status, &body).as_bytes())
                .await;
            let _ = stream.shutdown().await;
        }
        Behavior::Truncated {
            declared_length,
            body,
        } => {
            let response = format!(
                "HTTP/1.1 200 Test\r\nConnection: close\r\nContent-Length: {declared_length}\r\n\r\n{body}"
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    }
}

async fn read_request(stream: &mut (impl AsyncRead + Unpin)) -> Option<CapturedRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        let Some(head_end) = find_head_end(&buffer) else {
            continue;
        };
        let head = String::from_utf8_lossy(&buffer[..head_end]).into_owned();
        let content_length = content_length(&head);
        let body_start = head_end + 4;
        if buffer.len() < body_start + content_length {
            continue;
        }
        let body =
            String::from_utf8_lossy(&buffer[body_start..body_start + content_length]).into_owned();
        return Some(parse_request(&head, body));
    }
}

fn find_head_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_request(head: &str, body: String) -> CapturedRequest {
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default().to_owned();
    let headers = lines
        .filter_map(|line| {
            line.split_once(": ")
                .map(|(name, value)| (name.to_ascii_lowercase(), value.to_owned()))
        })
        .collect();
    CapturedRequest {
        method,
        target,
        headers,
        body,
    }
}

fn content_length(head: &str) -> usize {
    head.split("\r\n")
        .filter_map(|line| line.split_once(": "))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or(0)
}

fn response_with_headers(status: u16, body: &str, headers: &[(&'static str, String)]) -> String {
    let mut response = format!(
        "HTTP/1.1 {status} Test\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (name, value) in headers {
        response.push_str(name);
        response.push_str(": ");
        response.push_str(value);
        response.push_str("\r\n");
    }
    response.push_str("\r\n");
    response.push_str(body);
    response
}

fn chunked_response(status: u16, body: &str) -> String {
    let mut response = format!(
        "HTTP/1.1 {status} Test\r\nConnection: close\r\nTransfer-Encoding: chunked\r\n\r\n"
    );
    for chunk in body.as_bytes().chunks(16 * 1024) {
        response.push_str(&format!("{:x}\r\n", chunk.len()));
        response.push_str(std::str::from_utf8(chunk).expect("fixture body is ASCII"));
        response.push_str("\r\n");
    }
    response.push_str("0\r\n\r\n");
    response
}

/// The fixture connection either transports plain TCP or a TLS session.
enum TestStream {
    Plain(TcpStream),
    Tls(Box<TlsStream<TcpStream>>),
}

impl AsyncRead for TestStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_read(context, buffer),
            Self::Tls(stream) => Pin::new(stream).poll_read(context, buffer),
        }
    }
}

impl AsyncWrite for TestStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_write(context, bytes),
            Self::Tls(stream) => Pin::new(stream).poll_write(context, bytes),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_flush(context),
            Self::Tls(stream) => Pin::new(stream).poll_flush(context),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_shutdown(context),
            Self::Tls(stream) => Pin::new(stream).poll_shutdown(context),
        }
    }
}

/// A purpose-built PKI: one CA plus a server leaf valid for `localhost` and
/// `127.0.0.1`, an unrelated CA, and a leaf with the wrong DNS name.
pub(crate) struct TestPki {
    pub(crate) server_config: Arc<rustls::ServerConfig>,
    pub(crate) wrong_name_config: Arc<rustls::ServerConfig>,
    pub(crate) ca_pem: Vec<u8>,
    pub(crate) other_ca_pem: Vec<u8>,
}

pub(crate) fn test_pki() -> TestPki {
    let (ca_pem, ca_issuer) = authority("Test CA");
    let (other_ca_pem, _) = authority("Other CA");
    TestPki {
        server_config: server_config(&ca_issuer, &["localhost"], &["127.0.0.1"]),
        wrong_name_config: server_config(&ca_issuer, &["wrong.example"], &[]),
        ca_pem: ca_pem.into_bytes(),
        other_ca_pem: other_ca_pem.into_bytes(),
    }
}

fn authority(common_name: &str) -> (String, Issuer<'static, KeyPair>) {
    let mut parameters = CertificateParams::new(Vec::new()).expect("CA params");
    parameters
        .distinguished_name
        .push(DnType::CommonName, common_name);
    parameters.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let key_pair = KeyPair::generate().expect("CA key");
    let certificate = parameters.self_signed(&key_pair).expect("CA self-signs");
    let pem = certificate.pem();
    (pem, Issuer::new(parameters, key_pair))
}

fn server_config(
    issuer: &Issuer<'static, KeyPair>,
    dns_names: &[&str],
    ip_addresses: &[&str],
) -> Arc<rustls::ServerConfig> {
    use rustls::pki_types::pem::PemObject;

    let mut parameters = CertificateParams::new(
        dns_names
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>(),
    )
    .expect("leaf params");
    parameters
        .distinguished_name
        .push(DnType::CommonName, "fixture-server");
    parameters.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    parameters.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    for address in ip_addresses {
        parameters
            .subject_alt_names
            .push(SanType::IpAddress(address.parse().expect("fixture IP SAN")));
    }
    let key_pair = KeyPair::generate().expect("leaf key");
    let certificate = parameters
        .signed_by(&key_pair, issuer)
        .expect("leaf is signed");
    let chain: Vec<_> =
        rustls::pki_types::CertificateDer::pem_slice_iter(certificate.pem().as_bytes())
            .collect::<Result<_, _>>()
            .expect("leaf chain parses");
    let private_key =
        rustls::pki_types::PrivateKeyDer::from_pem_slice(key_pair.serialize_pem().as_bytes())
            .expect("leaf key parses");
    // Explicit ring provider: provider auto-detect panics and stays out of
    // every construction path, test and production alike.
    Arc::new(
        rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(rustls::ALL_VERSIONS)
        .expect("protocol versions")
        .with_no_client_auth()
        .with_single_cert(chain, private_key)
        .expect("fixture server config"),
    )
}

/// A client for one fixture origin with the given trust and deadline.
pub(crate) fn client_for(
    origin: &str,
    trust: TrustPolicy,
    deadline: Option<Duration>,
) -> NativeClient {
    let mut config = NativeClientConfig::new(origin);
    config.trust = trust;
    if let Some(deadline) = deadline {
        config.request_deadline = deadline;
    }
    NativeClient::new(config).expect("fixture client builds")
}

/// A plaintext client for a loopback fixture server.
pub(crate) fn plain_client(server: &TestServer) -> NativeClient {
    client_for(&server.origin(), TrustPolicy::WebPki, None)
}

/// A TLS client trusting only the fixture CA.
pub(crate) fn tls_client(server: &TestServer, ca_pem: &[u8]) -> NativeClient {
    client_for(
        &server.origin(),
        TrustPolicy::PemBundle(ca_pem.to_vec()),
        None,
    )
}

/// The checked canonical OpenAPI artifact.
pub(crate) fn openapi_artifact() -> serde_json::Value {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/openapi/openapi.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("checked artifact is missing: {}", path.display()));
    serde_json::from_str(&text).expect("checked artifact is JSON")
}

/// One covered subset of the artifact: only the named operations, their
/// component schemas, and the security scheme definitions.
pub(crate) fn covered_contract(
    document: &serde_json::Value,
    paths: &[(&str, &str)],
    schemas: &[&str],
) -> serde_json::Value {
    let mut covered_paths = serde_json::Map::new();
    for (path, method) in paths {
        let operation = document["paths"][path][method].clone();
        assert!(operation.is_object(), "{method} {path} must be documented");
        let mut methods = serde_json::Map::new();
        methods.insert((*method).to_owned(), operation);
        covered_paths.insert((*path).to_owned(), serde_json::Value::Object(methods));
    }
    let mut covered_schemas = serde_json::Map::new();
    for name in schemas {
        let schema = document["components"]["schemas"][name].clone();
        assert!(schema.is_object(), "{name} must be documented");
        covered_schemas.insert((*name).to_owned(), schema);
    }
    serde_json::json!({
        "paths": serde_json::Value::Object(covered_paths),
        "schemas": serde_json::Value::Object(covered_schemas),
        "securitySchemes": document["components"]["securitySchemes"].clone(),
    })
}

/// Compare one covered contract against its checked snapshot. Regenerate with
/// `UPDATE_NATIVE_CLIENT_SNAPSHOTS=1` and review the diff.
pub(crate) fn assert_openapi_snapshot(relative: &str, actual: &serde_json::Value) {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/tests/fixtures")
        .join(relative);
    let rendered = serde_json::to_string_pretty(actual).expect("snapshot serializes") + "\n";
    if std::env::var("UPDATE_NATIVE_CLIENT_SNAPSHOTS").as_deref() == Ok("1") {
        std::fs::write(&path, &rendered).expect("snapshot writes");
        return;
    }
    let checked = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("checked snapshot is missing: {}", path.display()));
    assert_eq!(
        checked, rendered,
        "checked snapshot drifted; regenerate with UPDATE_NATIVE_CLIENT_SNAPSHOTS=1 and review"
    );
}

pub(crate) fn schema_fields(
    document: &serde_json::Value,
    name: &str,
) -> std::collections::BTreeSet<String> {
    document["components"]["schemas"][name]["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{name} documents properties"))
        .keys()
        .cloned()
        .collect()
}

pub(crate) fn serialized_keys<T: serde::Serialize>(
    value: &T,
) -> std::collections::BTreeSet<String> {
    serde_json::to_value(value)
        .expect("wire DTO serializes")
        .as_object()
        .expect("wire DTO serializes to an object")
        .keys()
        .cloned()
        .collect()
}

pub(crate) fn round_trip_keys<T: serde::de::DeserializeOwned + serde::Serialize>(
    fixture: &str,
) -> std::collections::BTreeSet<String> {
    let value: T = serde_json::from_str(fixture).expect("fixture decodes into the wire DTO");
    serialized_keys(&value)
}

pub(crate) fn assert_schema_fields(
    document: &serde_json::Value,
    name: &str,
    keys: std::collections::BTreeSet<String>,
) {
    assert_eq!(
        schema_fields(document, name),
        keys,
        "{name} properties must match the client wire DTO"
    );
}

pub(crate) fn response_statuses(
    document: &serde_json::Value,
    path: &str,
    method: &str,
) -> std::collections::BTreeSet<String> {
    document["paths"][path][method]["responses"]
        .as_object()
        .unwrap_or_else(|| panic!("{method} {path} documents responses"))
        .keys()
        .cloned()
        .collect()
}
