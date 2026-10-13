mod support;

use std::{
    future::pending,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use axum::{Router, routing::get};
use kontainer_agent_proto::{
    PROTOCOL_PACKAGE,
    v1::{
        CheckInRequest, DescribeRequest, ListContainersRequest,
        controller_intake_client::ControllerIntakeClient, node_agent_client::NodeAgentClient,
    },
};
use kontainer_controller::rpc::{AgentClientError, NodeAgentClientConfig};
use kontainer_controller::rpc::{ControllerIntakeService, serve_controller_rpc};
use kontainer_controller::server::{
    BoundAgentRpcServer, BoundHttpServer, ControllerShutdown, HttpTransport,
    supervise_controller_servers,
};
use kontainer_engine::{
    ContainerEngine, ContainerPage, ContainerPageRequest, ContainerState, ContainerSummary,
    EngineError, EngineErrorKind, EngineOperation,
};
use kontainer_lifecycle::ShutdownGrace;
use kontainer_node_agent::{
    AgentShutdown, NodeAgentService, serve_node_agent_rpc, supervise_node_agent_server,
};
use kontainer_rpc_auth::{
    AGENT_DNS_SUFFIX, CONTROLLER_DNS_NAME, ClusterSecret, DEFAULT_CLUSTER_ID,
    MutualTlsIdentityFiles, WorkloadIdentity,
};
use support::pki::{
    AGENT_URI, CLUSTER_ID, CONTROLLER_NAME, CONTROLLER_URI, NODE_AGENT_NAME, NODE_ID,
    OTHER_CLUSTER_ID, TestPki,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Semaphore,
    task::JoinHandle,
    time::timeout,
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tonic::{
    Code, Request,
    transport::{Channel, ClientTlsConfig, Endpoint},
};

type ServerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

fn unused_failure_sender() -> tokio::sync::mpsc::UnboundedSender<&'static str> {
    tokio::sync::mpsc::unbounded_channel().0
}

struct AvailableEngine;

#[async_trait]
impl ContainerEngine for AvailableEngine {
    async fn is_available(&self) -> Result<bool, EngineError> {
        Ok(true)
    }
}

struct InventoryEngine {
    pages: Vec<ContainerPage>,
    calls: Arc<AtomicUsize>,
    error: Option<(EngineErrorKind, EngineOperation)>,
}

#[async_trait]
impl ContainerEngine for InventoryEngine {
    async fn is_available(&self) -> Result<bool, EngineError> {
        Ok(true)
    }

    async fn list_containers(
        &self,
        _request: ContainerPageRequest,
        _cancellation: &CancellationToken,
    ) -> Result<ContainerPage, EngineError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some((kind, operation)) = self.error {
            return Err(EngineError::new(kind, operation));
        }
        let index = self.calls.load(Ordering::SeqCst) - 1;
        Ok(self.pages[index.min(self.pages.len() - 1)].clone())
    }
}

fn inventory_summary(id: &str, name: &str) -> ContainerSummary {
    ContainerSummary {
        container_id: id.to_owned(),
        display_name: name.to_owned(),
        image_reference: "nginx:latest".to_owned(),
        state: ContainerState::Running,
        status: "Up 3s".to_owned(),
        created_at_seconds: 1_700_000_000,
        ..ContainerSummary::default()
    }
}

struct BlockingEngine {
    entered: Arc<Semaphore>,
}

#[async_trait]
impl ContainerEngine for BlockingEngine {
    async fn is_available(&self) -> Result<bool, EngineError> {
        self.entered.add_permits(1);
        pending().await
    }
}

struct ReleasableEngine {
    entered: Arc<Semaphore>,
    release: Arc<Semaphore>,
}

struct PanicEngine;

#[async_trait]
impl ContainerEngine for PanicEngine {
    async fn is_available(&self) -> Result<bool, EngineError> {
        panic!("engine failed")
    }
}

#[async_trait]
impl ContainerEngine for ReleasableEngine {
    async fn is_available(&self) -> Result<bool, EngineError> {
        self.entered.add_permits(1);
        self.release.acquire().await.unwrap().forget();
        Ok(true)
    }
}

struct BlockingInventoryEngine {
    entered: Arc<Semaphore>,
}

#[async_trait]
impl ContainerEngine for BlockingInventoryEngine {
    async fn is_available(&self) -> Result<bool, EngineError> {
        Ok(true)
    }

    async fn list_containers(
        &self,
        _request: ContainerPageRequest,
        _cancellation: &CancellationToken,
    ) -> Result<ContainerPage, EngineError> {
        self.entered.add_permits(1);
        pending().await
    }
}

struct RunningServer {
    address: std::net::SocketAddr,
    cancellation: CancellationToken,
    task: JoinHandle<ServerResult>,
}

impl RunningServer {
    async fn stop(self) {
        self.cancellation.cancel();
        timeout(Duration::from_secs(2), self.task)
            .await
            .expect("RPC server did not stop within its shutdown budget")
            .expect("RPC server task panicked")
            .expect("RPC server returned an error");
    }
}

#[tokio::test]
async fn describe_reports_node_identity_and_engine_health() {
    let pki = TestPki::generate();
    let node_agent = start_node_agent(&pki, Arc::new(AvailableEngine)).await;
    let mut agent = node_agent_client(node_agent.address, pki.trusted_files(&pki.controller)).await;

    let response = agent
        .describe(DescribeRequest {})
        .await
        .unwrap()
        .into_inner();

    assert_eq!(response.node_id, NODE_ID);
    assert_eq!(response.agent_version, env!("CARGO_PKG_VERSION"));
    assert!(response.docker_connected);
    assert!(!response.architecture.is_empty());

    node_agent.stop().await;
}

/// The certificates `dev/pki.sh` writes must satisfy the same rpc-auth rules
/// as the generated fixtures. Runs when `KONTAINER_TEST_DEV_PKI_DIR` points at
/// that directory (for example `../dev/pki`).
#[tokio::test]
async fn dev_pki_files_complete_a_verified_describe() {
    let Ok(directory) = std::env::var("KONTAINER_TEST_DEV_PKI_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    let files = |stem: &str| {
        MutualTlsIdentityFiles::new(
            directory.join(format!("{stem}.pem")),
            directory.join(format!("{stem}-key.pem")),
            directory.join("agent-ca.pem"),
        )
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let cancellation = CancellationToken::new();
    let service = NodeAgentService::new("manager", Arc::new(AvailableEngine), cancellation.clone());
    let server = serve_node_agent_rpc(
        listener,
        "development".to_owned(),
        files("node-manager"),
        service,
        cancellation.clone(),
        cancellation.clone(),
        TaskTracker::new(),
        TaskTracker::new(),
        unused_failure_sender(),
    )
    .unwrap();
    let task = tokio::spawn(server);

    let client = NodeAgentClientConfig::new(
        format!("https://{address}"),
        "manager.agent.kontainer.internal".to_owned(),
        "development".to_owned(),
        "manager".to_owned(),
        files("controller"),
    )
    .unwrap();
    let description = client.describe(&CancellationToken::new()).await.unwrap();
    assert_eq!(description.node_id, "manager");

    let wrong_node = NodeAgentClientConfig::new(
        format!("https://{address}"),
        "manager.agent.kontainer.internal".to_owned(),
        "development".to_owned(),
        "worker-1".to_owned(),
        files("controller"),
    )
    .unwrap();
    assert!(
        wrong_node
            .describe(&CancellationToken::new())
            .await
            .is_err()
    );

    cancellation.cancel();
    let _ = timeout(Duration::from_secs(2), task).await;
}

/// Each side issues its own certificate from the shared secret (ADR 0045); the unchanged mTLS
/// checks accept the pair, and a controller holding another secret is refused.
#[tokio::test]
async fn secret_issued_identities_complete_a_verified_describe() {
    let secret = |digit: &str| ClusterSecret::new(&digit.repeat(64)).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let issue = |secret: ClusterSecret, name: &str, identity: WorkloadIdentity, dns: &str| {
        secret
            .issue_identity_files(&identity, dns, &directory.path().join(name))
            .unwrap()
    };
    let controller = |secret: ClusterSecret, name: &str| {
        issue(
            secret,
            name,
            WorkloadIdentity::Controller {
                cluster_id: DEFAULT_CLUSTER_ID.to_owned(),
                instance_id: "controller".to_owned(),
            },
            CONTROLLER_DNS_NAME,
        )
    };
    let agent_files = issue(
        secret("a"),
        "agent",
        WorkloadIdentity::NodeAgent {
            cluster_id: DEFAULT_CLUSTER_ID.to_owned(),
            node_id: "tj42rq5nzwf8".to_owned(),
        },
        &format!("tj42rq5nzwf8.{AGENT_DNS_SUFFIX}"),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let cancellation = CancellationToken::new();
    let service = NodeAgentService::new(
        "tj42rq5nzwf8",
        Arc::new(AvailableEngine),
        cancellation.clone(),
    );
    let server = serve_node_agent_rpc(
        listener,
        DEFAULT_CLUSTER_ID.to_owned(),
        agent_files,
        service,
        cancellation.clone(),
        cancellation.clone(),
        TaskTracker::new(),
        TaskTracker::new(),
        unused_failure_sender(),
    )
    .unwrap();
    let task = tokio::spawn(server);

    let client = |files| {
        NodeAgentClientConfig::new(
            format!("https://{address}"),
            format!("tj42rq5nzwf8.{AGENT_DNS_SUFFIX}"),
            DEFAULT_CLUSTER_ID.to_owned(),
            "tj42rq5nzwf8".to_owned(),
            files,
        )
        .unwrap()
    };
    let description = client(controller(secret("a"), "controller"))
        .describe(&CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(description.node_id, "tj42rq5nzwf8");

    assert!(
        client(controller(secret("b"), "stranger"))
            .describe(&CancellationToken::new())
            .await
            .is_err()
    );

    cancellation.cancel();
    let _ = timeout(Duration::from_secs(2), task).await;
}

/// Stops that wait out their grace take longer than a read's RPC timeout.
struct SlowStopEngine;

#[async_trait]
impl ContainerEngine for SlowStopEngine {
    async fn is_available(&self) -> Result<bool, EngineError> {
        Ok(true)
    }

    async fn container_action(
        &self,
        container_id: &str,
        _action: kontainer_engine::ContainerAction,
        _options: kontainer_engine::ContainerActionOptions,
        _cancellation: &CancellationToken,
    ) -> Result<(), EngineError> {
        if container_id.starts_with('b') {
            return Err(EngineError::new(
                EngineErrorKind::Conflict,
                EngineOperation::ContainerAction,
            ));
        }
        tokio::time::sleep(Duration::from_millis(2_500)).await;
        Ok(())
    }
}

#[tokio::test]
async fn production_client_actions_outlast_the_read_timeout_and_keep_their_codes() {
    use kontainer_controller::rpc::AgentContainerAction;

    let pki = TestPki::generate();
    let node_agent = start_node_agent(&pki, Arc::new(SlowStopEngine)).await;
    let client = production_client(
        node_agent.address,
        pki.trusted_files(&pki.controller),
        NODE_AGENT_NAME,
        CLUSTER_ID,
        NODE_ID,
    );
    client
        .container_action(
            &"a".repeat(64),
            AgentContainerAction::Stop,
            false,
            Some(1),
            &CancellationToken::new(),
        )
        .await
        .expect("a stop that takes longer than two seconds still succeeds");
    assert_eq!(
        client
            .container_action(
                &"b".repeat(64),
                AgentContainerAction::Remove,
                false,
                None,
                &CancellationToken::new()
            )
            .await
            .unwrap_err(),
        AgentClientError::ContainerRunning
    );
    node_agent.stop().await;
}

#[tokio::test]
async fn production_client_describes_the_configured_agent() {
    let pki = TestPki::generate();
    let node_agent = start_node_agent(&pki, Arc::new(AvailableEngine)).await;
    let client = production_client(
        node_agent.address,
        pki.trusted_files(&pki.controller),
        NODE_AGENT_NAME,
        CLUSTER_ID,
        NODE_ID,
    );

    let description = client.describe(&CancellationToken::new()).await.unwrap();

    assert_eq!(description.node_id, NODE_ID);
    assert_eq!(description.agent_version, env!("CARGO_PKG_VERSION"));
    assert!(description.docker_connected);
    assert!(!description.architecture.is_empty());

    node_agent.stop().await;
}

#[tokio::test]
async fn production_client_rejects_wrong_workload_cluster_and_node_before_rpc() {
    let pki = TestPki::generate();
    // Wrong workload: server presents a controller identity where a node
    // agent is expected.
    let wrong_workload = start_node_agent_with_tls(
        &pki,
        pki.trusted_files(&pki.wrong_workload),
        NODE_ID,
        Arc::new(AvailableEngine),
    )
    .await;
    let client = production_client(
        wrong_workload.address,
        pki.trusted_files(&pki.controller),
        "wrong-workload.kontainer.test",
        CLUSTER_ID,
        NODE_ID,
    );
    // The server DNS matches the presented certificate, so WebPKI passes and
    // the failure must come from the workload-identity check.
    let error = client
        .describe(&CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        AgentClientError::Unavailable | AgentClientError::IdentityMismatch
    ));
    wrong_workload.stop().await;

    // Wrong cluster: same node, different cluster.
    let wrong_cluster = start_node_agent_with_tls(
        &pki,
        pki.trusted_files(&pki.wrong_cluster),
        NODE_ID,
        Arc::new(AvailableEngine),
    )
    .await;
    let client = production_client(
        wrong_cluster.address,
        pki.trusted_files(&pki.controller),
        "wrong-cluster.kontainer.test",
        CLUSTER_ID,
        NODE_ID,
    );
    let error = client
        .describe(&CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        AgentClientError::Unavailable | AgentClientError::IdentityMismatch
    ));
    wrong_cluster.stop().await;

    // Wrong node: the certificate is valid for node-1, but the controller is
    // statically configured for node-2.
    let node_agent = start_node_agent(&pki, Arc::new(AvailableEngine)).await;
    let client = production_client(
        node_agent.address,
        pki.trusted_files(&pki.controller),
        NODE_AGENT_NAME,
        CLUSTER_ID,
        "node-2",
    );
    let error = client
        .describe(&CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        AgentClientError::Unavailable | AgentClientError::IdentityMismatch
    ));
    node_agent.stop().await;
}

#[tokio::test]
async fn production_client_rejects_unknown_ca_wrong_dns_and_plaintext() {
    let pki = TestPki::generate();

    // Unknown CA: server certificate is signed by an untrusted CA.
    let untrusted = start_node_agent_with_tls(
        &pki,
        pki.trusted_files(&pki.untrusted_agent),
        NODE_ID,
        Arc::new(AvailableEngine),
    )
    .await;
    let client = production_client(
        untrusted.address,
        pki.trusted_files(&pki.controller),
        "untrusted-agent.kontainer.test",
        CLUSTER_ID,
        NODE_ID,
    );
    let error = client
        .describe(&CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error, AgentClientError::Unavailable);
    assert_eq!(error.to_string(), "agent is unavailable");
    untrusted.stop().await;

    // Wrong DNS name: the certificate is trusted, but the expected server
    // name does not match its DNS SAN.
    let node_agent = start_node_agent(&pki, Arc::new(AvailableEngine)).await;
    let node_address = node_agent.address;
    let client = production_client(
        node_address,
        pki.trusted_files(&pki.controller),
        "wrong-dns.kontainer.test",
        CLUSTER_ID,
        NODE_ID,
    );
    let error = client
        .describe(&CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error, AgentClientError::Unavailable);
    node_agent.stop().await;

    // Plaintext endpoints never validate: the static endpoint must be https.
    let tls_files = pki.trusted_files(&pki.controller);
    let plaintext = NodeAgentClientConfig::new(
        format!("http://{node_address}"),
        NODE_AGENT_NAME.to_owned(),
        CLUSTER_ID.to_owned(),
        NODE_ID.to_owned(),
        tls_files,
    )
    .unwrap_err();
    assert_eq!(plaintext.to_string(), "agent endpoint is invalid");

    // Missing client TLS files fail construction, not the first call.
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.pem");
    let missing_files = MutualTlsIdentityFiles::new(&missing, &missing, &missing);
    let error = NodeAgentClientConfig::new(
        format!("https://{node_address}"),
        NODE_AGENT_NAME.to_owned(),
        CLUSTER_ID.to_owned(),
        NODE_ID.to_owned(),
        missing_files,
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("agent client TLS files are invalid")
    );

    // An unreachable agent constructs successfully; the failure is a typed
    // call error rather than a startup failure.
    let unreachable_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let unreachable = unreachable_listener.local_addr().unwrap();
    drop(unreachable_listener);
    let client = production_client(
        unreachable,
        pki.trusted_files(&pki.controller),
        NODE_AGENT_NAME,
        CLUSTER_ID,
        NODE_ID,
    );
    let error = client
        .describe(&CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error, AgentClientError::Unavailable);
}

#[tokio::test]
async fn production_client_rejects_a_mismatched_reported_node_id() {
    let pki = TestPki::generate();
    // TLS passes (the certificate is node-1), but the handler reports
    // node-2. The client must not trust the protobuf claim.
    let node_agent = start_node_agent_with_tls(
        &pki,
        pki.trusted_files(&pki.agent),
        "node-2",
        Arc::new(AvailableEngine),
    )
    .await;
    let client = production_client(
        node_agent.address,
        pki.trusted_files(&pki.controller),
        NODE_AGENT_NAME,
        CLUSTER_ID,
        NODE_ID,
    );

    let error = client
        .describe(&CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error, AgentClientError::IdentityMismatch);
    assert_eq!(error.to_string(), "agent identity did not match");

    node_agent.stop().await;
}

#[tokio::test]
async fn production_client_cancellation_wins_without_detached_work() {
    let pki = TestPki::generate();
    let entered = Arc::new(Semaphore::new(0));
    let node_agent = start_node_agent(
        &pki,
        Arc::new(BlockingEngine {
            entered: entered.clone(),
        }),
    )
    .await;
    let client = production_client(
        node_agent.address,
        pki.trusted_files(&pki.controller),
        NODE_AGENT_NAME,
        CLUSTER_ID,
        NODE_ID,
    );
    let cancellation = CancellationToken::new();
    let call_cancellation = cancellation.clone();
    let call = tokio::spawn(async move { client.describe(&call_cancellation).await });

    entered.acquire().await.unwrap().forget();
    cancellation.cancel();

    let error = timeout(Duration::from_secs(2), call)
        .await
        .expect("in-flight Describe did not observe cancellation")
        .expect("Describe task panicked")
        .unwrap_err();
    assert_eq!(error, AgentClientError::Cancelled);

    // Caller cancellation ends the client call; the server still shuts down
    // cleanly afterwards, proving no detached client work keeps it alive.
    node_agent.stop().await;
}

#[tokio::test]
async fn production_client_connect_deadline_expiry_is_a_timeout() {
    let pki = TestPki::generate();
    // Accept TCP but never complete the TLS handshake, so the client's own
    // connect deadline is the only way out. A surfaced transport error here
    // would misreport a deadline expiry as unavailability.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let stall = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let _held = socket;
        pending::<()>().await;
    });

    let client = production_client(
        address,
        pki.trusted_files(&pki.controller),
        NODE_AGENT_NAME,
        CLUSTER_ID,
        NODE_ID,
    );
    let error = client
        .describe(&CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error, AgentClientError::TimedOut);

    stall.abort();
}

#[tokio::test]
async fn production_client_lists_bounded_pages_with_provenance() {
    let pki = TestPki::generate();
    let first_id = "a".repeat(64);
    let second_id = "b".repeat(64);
    let cursor = format!("v1:{first_id}");
    let engine = Arc::new(InventoryEngine {
        pages: vec![
            ContainerPage {
                containers: vec![inventory_summary(&first_id, "web")],
                next_page_token: Some(cursor.clone()),
            },
            ContainerPage {
                containers: vec![inventory_summary(&second_id, "worker")],
                next_page_token: None,
            },
        ],
        calls: Arc::new(AtomicUsize::new(0)),
        error: None,
    });
    let calls = engine.calls.clone();
    let node_agent = start_node_agent(&pki, engine).await;
    let client = production_client(
        node_agent.address,
        pki.trusted_files(&pki.controller),
        NODE_AGENT_NAME,
        CLUSTER_ID,
        NODE_ID,
    );

    let first = client
        .list_containers(1, String::new(), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(first.containers.len(), 1);
    assert_eq!(first.containers[0].container_id, first_id);
    assert_eq!(first.containers[0].display_name, "web");
    assert_eq!(first.containers[0].node_id, NODE_ID);
    assert_eq!(first.next_page_token.as_deref(), Some(cursor.as_str()));

    let second = client
        .list_containers(1, cursor, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(second.containers.len(), 1);
    assert_eq!(second.containers[0].container_id, second_id);
    assert!(second.next_page_token.is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    node_agent.stop().await;
}

#[tokio::test]
async fn production_client_maps_pagination_and_daemon_failures_without_internals() {
    let pki = TestPki::generate();
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = Arc::new(InventoryEngine {
        pages: vec![ContainerPage {
            containers: vec![],
            next_page_token: None,
        }],
        calls: calls.clone(),
        error: None,
    });
    let node_agent = start_node_agent(&pki, engine).await;
    let client = production_client(
        node_agent.address,
        pki.trusted_files(&pki.controller),
        NODE_AGENT_NAME,
        CLUSTER_ID,
        NODE_ID,
    );

    // Oversized and malformed pagination never reach Engine access: the agent
    // rejects them as invalid input, which the client preserves as a typed
    // pagination failure.
    for (size, token) in [
        (501, String::new()),
        (10, "not-a-cursor".to_owned()),
        (10, "v1:short".to_owned()),
    ] {
        let error = client
            .list_containers(size, token, &CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(error, AgentClientError::InvalidPagination);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    node_agent.stop().await;

    // Daemon failures stay redacted unavailable errors.
    let failing = start_node_agent(
        &pki,
        Arc::new(InventoryEngine {
            pages: vec![],
            calls: Arc::new(AtomicUsize::new(0)),
            error: Some((
                EngineErrorKind::DaemonRejected,
                EngineOperation::ListContainers,
            )),
        }),
    )
    .await;
    let client = production_client(
        failing.address,
        pki.trusted_files(&pki.controller),
        NODE_AGENT_NAME,
        CLUSTER_ID,
        NODE_ID,
    );
    let error = client
        .list_containers(10, String::new(), &CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error, AgentClientError::Unavailable);
    assert_eq!(error.to_string(), "agent is unavailable");
    failing.stop().await;
}

#[tokio::test]
async fn production_client_list_cancellation_wins_without_detached_work() {
    let pki = TestPki::generate();
    let entered = Arc::new(Semaphore::new(0));
    let node_agent = start_node_agent(
        &pki,
        Arc::new(BlockingInventoryEngine {
            entered: entered.clone(),
        }),
    )
    .await;
    let client = production_client(
        node_agent.address,
        pki.trusted_files(&pki.controller),
        NODE_AGENT_NAME,
        CLUSTER_ID,
        NODE_ID,
    );
    let cancellation = CancellationToken::new();
    let call_cancellation = cancellation.clone();
    let call = tokio::spawn(async move {
        client
            .list_containers(10, String::new(), &call_cancellation)
            .await
    });

    entered.acquire().await.unwrap().forget();
    cancellation.cancel();

    let error = timeout(Duration::from_secs(2), call)
        .await
        .expect("in-flight listing did not observe cancellation")
        .expect("listing task panicked")
        .unwrap_err();
    assert_eq!(error, AgentClientError::Cancelled);

    node_agent.stop().await;
}

#[tokio::test]
async fn list_containers_serves_bounded_pages_with_node_provenance() {
    let pki = TestPki::generate();
    let first_id = "a".repeat(64);
    let second_id = "b".repeat(64);
    let cursor = format!("v1:{first_id}");
    let engine = Arc::new(InventoryEngine {
        pages: vec![
            ContainerPage {
                containers: vec![inventory_summary(&first_id, "web")],
                next_page_token: Some(cursor.clone()),
            },
            ContainerPage {
                containers: vec![inventory_summary(&second_id, "worker")],
                next_page_token: None,
            },
        ],
        calls: Arc::new(AtomicUsize::new(0)),
        error: None,
    });
    let calls = engine.calls.clone();
    let node_agent = start_node_agent(&pki, engine).await;
    let mut agent = node_agent_client(node_agent.address, pki.trusted_files(&pki.controller)).await;

    let first = agent
        .list_containers(ListContainersRequest {
            page_size: 1,
            page_token: String::new(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(first.containers.len(), 1);
    assert_eq!(first.containers[0].container_id, first_id);
    assert_eq!(first.containers[0].display_name, "web");
    assert_eq!(first.containers[0].node_id, NODE_ID);
    assert_eq!(first.next_page_token, cursor);

    let second = agent
        .list_containers(ListContainersRequest {
            page_size: 1,
            page_token: cursor,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(second.containers.len(), 1);
    assert_eq!(second.containers[0].container_id, second_id);
    assert_eq!(second.containers[0].node_id, NODE_ID);
    assert!(second.next_page_token.is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    // The private summary carries no environment, command, label, mount,
    // port, or raw inspect data.
    let rendered = format!("{:?}", second.containers[0]);
    assert!(!rendered.contains("secret-canary"));

    node_agent.stop().await;
}

#[tokio::test]
async fn list_containers_rejects_invalid_pagination_and_agent_identity() {
    let pki = TestPki::generate();
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = Arc::new(InventoryEngine {
        pages: vec![ContainerPage {
            containers: vec![],
            next_page_token: None,
        }],
        calls: calls.clone(),
        error: None,
    });
    let node_agent = start_node_agent(&pki, engine).await;

    let mut agent = node_agent_client(node_agent.address, pki.trusted_files(&pki.controller)).await;
    for token in ["not-a-cursor", "v1:short"] {
        let error = agent
            .list_containers(ListContainersRequest {
                page_size: 10,
                page_token: token.to_owned(),
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), Code::InvalidArgument);
    }
    let oversized = agent
        .list_containers(ListContainersRequest {
            page_size: 501,
            page_token: String::new(),
        })
        .await
        .unwrap_err();
    assert_eq!(oversized.code(), Code::InvalidArgument);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    // An agent certificate used as a controller identity is rejected before
    // any Engine access.
    let mut impostor = node_agent_client(node_agent.address, pki.trusted_files(&pki.agent)).await;
    let error = impostor
        .list_containers(ListContainersRequest {
            page_size: 10,
            page_token: String::new(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::Unauthenticated);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    node_agent.stop().await;
}

#[tokio::test]
async fn list_containers_maps_daemon_failure_without_internals() {
    let pki = TestPki::generate();
    let node_agent = start_node_agent(
        &pki,
        Arc::new(InventoryEngine {
            pages: vec![],
            calls: Arc::new(AtomicUsize::new(0)),
            error: Some((
                EngineErrorKind::DaemonRejected,
                EngineOperation::ListContainers,
            )),
        }),
    )
    .await;
    let mut agent = node_agent_client(node_agent.address, pki.trusted_files(&pki.controller)).await;

    let error = agent
        .list_containers(ListContainersRequest {
            page_size: 10,
            page_token: String::new(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::Unavailable);
    assert!(!error.message().contains("secret-canary"));
    assert!(!error.message().contains("/var/run/docker.sock"));

    node_agent.stop().await;
}

#[tokio::test]
async fn valid_controller_and_agent_complete_both_versioned_rpc_exchanges() {
    let pki = TestPki::generate();
    let controller = start_controller(&pki).await;
    let node_agent = start_node_agent(&pki, Arc::new(AvailableEngine)).await;

    let mut intake = controller_client(controller.address, pki.trusted_files(&pki.agent)).await;
    let check_in = intake
        .check_in(CheckInRequest {
            agent_id: NODE_ID.to_owned(),
            agent_version: "0.1.0-test".to_owned(),
            architecture: std::env::consts::ARCH.to_owned(),
            ..CheckInRequest::default()
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(check_in.protocol_package, PROTOCOL_PACKAGE);

    let mut agent = node_agent_client(node_agent.address, pki.trusted_files(&pki.controller)).await;
    let description = agent
        .describe(DescribeRequest {})
        .await
        .unwrap()
        .into_inner();
    assert_eq!(description.node_id, NODE_ID);
    assert!(description.docker_connected);

    eprintln!(
        "mTLS evidence: protocol={PROTOCOL_PACKAGE} controller={CONTROLLER_URI} agent={AGENT_URI} controller_port={} agent_port={} result=accepted",
        controller.address.port(),
        node_agent.address.port()
    );

    controller.stop().await;
    node_agent.stop().await;
}

#[tokio::test]
async fn controller_rejects_missing_untrusted_wrong_cluster_and_wrong_workload_identities() {
    let pki = TestPki::generate();
    let controller = start_controller(&pki).await;

    assert_controller_exchange_rejected(
        controller.address,
        pki.anonymous_client_config(CONTROLLER_NAME),
        "missing-client-certificate",
    )
    .await;
    assert_controller_exchange_rejected(
        controller.address,
        pki.untrusted_agent_files()
            .client_config(CONTROLLER_NAME)
            .unwrap(),
        "unknown-ca",
    )
    .await;
    assert_controller_plaintext_rejected(controller.address).await;

    let wrong_cluster_error =
        controller_client(controller.address, pki.trusted_files(&pki.wrong_cluster))
            .await
            .check_in(check_in_request(NODE_ID))
            .await
            .unwrap_err();
    assert_eq!(wrong_cluster_error.code(), Code::Unauthenticated);

    let wrong_workload_error =
        controller_client(controller.address, pki.trusted_files(&pki.wrong_workload))
            .await
            .check_in(check_in_request(NODE_ID))
            .await
            .unwrap_err();
    assert_eq!(wrong_workload_error.code(), Code::Unauthenticated);

    let mismatched_claim_error =
        controller_client(controller.address, pki.trusted_files(&pki.agent))
            .await
            .check_in(check_in_request("node-2"))
            .await
            .unwrap_err();
    assert_eq!(mismatched_claim_error.code(), Code::PermissionDenied);

    eprintln!(
        "mTLS rejection evidence: port={} missing_cert=rejected unknown_ca=rejected plaintext=rejected wrong_cluster={OTHER_CLUSTER_ID}:rejected wrong_workload=controller-as-agent:rejected mismatched_claim=rejected",
        controller.address.port()
    );

    controller.stop().await;
}

#[tokio::test]
async fn node_agent_rejects_an_agent_certificate_used_as_a_controller_identity() {
    let pki = TestPki::generate();
    let node_agent = start_node_agent(&pki, Arc::new(AvailableEngine)).await;

    let error = node_agent_client(node_agent.address, pki.trusted_files(&pki.agent))
        .await
        .describe(DescribeRequest {})
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::Unauthenticated);

    node_agent.stop().await;
}

#[tokio::test]
async fn controller_rejects_trusted_certificates_without_one_unambiguous_workload_identity() {
    let pki = TestPki::generate();
    let controller = start_controller(&pki).await;

    for fixture in [&pki.missing_identity, &pki.ambiguous_identity] {
        let error = controller_client(controller.address, pki.trusted_files(fixture))
            .await
            .check_in(check_in_request(NODE_ID))
            .await
            .unwrap_err();
        assert_eq!(error.code(), Code::Unauthenticated);
    }

    controller.stop().await;
}

#[tokio::test]
async fn shutdown_cancels_an_in_flight_rpc_and_joins_the_server_task() {
    let pki = TestPki::generate();
    let entered = Arc::new(Semaphore::new(0));
    let node_agent = start_node_agent(
        &pki,
        Arc::new(BlockingEngine {
            entered: entered.clone(),
        }),
    )
    .await;
    let mut client =
        node_agent_client(node_agent.address, pki.trusted_files(&pki.controller)).await;
    let call = tokio::spawn(async move { client.describe(Request::new(DescribeRequest {})).await });

    entered.acquire().await.unwrap().forget();
    node_agent.cancellation.cancel();

    let error = timeout(Duration::from_secs(2), call)
        .await
        .expect("in-flight RPC did not observe shutdown")
        .expect("in-flight RPC task panicked")
        .expect_err("in-flight RPC unexpectedly succeeded");
    assert!(matches!(
        error.code(),
        Code::Cancelled | Code::Unavailable | Code::Unknown
    ));
    timeout(Duration::from_secs(2), node_agent.task)
        .await
        .expect("node-agent server did not join after cancellation")
        .expect("node-agent server task panicked")
        .expect("node-agent server returned an error");
}

#[tokio::test]
async fn controller_deadline_cancels_and_joins_a_stuck_http_handler() {
    struct Dropped(Arc<AtomicUsize>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
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
                let _guard = Dropped(dropped);
                entered.add_permits(1);
                pending::<&'static str>().await
            }
        }),
    );
    let shutdown = ControllerShutdown::new("60ms".parse::<ShutdownGrace>().unwrap());
    let task_shutdown = shutdown.clone();
    let task = tokio::spawn(async move {
        supervise_controller_servers(
            router,
            BoundHttpServer {
                listener,
                transport: HttpTransport::Plaintext,
            },
            None,
            task_shutdown,
        )
        .await
    });
    let mut client = TcpStream::connect(address).await.unwrap();
    client
        .write_all(b"GET /stuck HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    entered.acquire().await.unwrap().forget();
    shutdown.begin_drain();

    let result = timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
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
async fn node_deadline_cancels_and_joins_a_stuck_rpc() {
    let pki = TestPki::generate();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let entered = Arc::new(Semaphore::new(0));
    let shutdown = AgentShutdown::new("60ms".parse::<ShutdownGrace>().unwrap());
    let connections = TaskTracker::new();
    let requests = TaskTracker::new();
    let service = NodeAgentService::new(
        NODE_ID,
        Arc::new(BlockingEngine {
            entered: entered.clone(),
        }),
        shutdown.force(),
    );
    let (failures, task_failures) = tokio::sync::mpsc::unbounded_channel();
    let server = serve_node_agent_rpc(
        listener,
        CLUSTER_ID.to_owned(),
        pki.trusted_files(&pki.agent),
        service,
        shutdown.draining(),
        shutdown.force(),
        connections.clone(),
        requests.clone(),
        failures,
    )
    .unwrap();
    let task_shutdown = shutdown.clone();
    let task_connections = connections.clone();
    let task_requests = requests.clone();
    let task = tokio::spawn(async move {
        supervise_node_agent_server(
            server,
            task_shutdown,
            task_connections,
            task_requests,
            task_failures,
            pending(),
        )
        .await
    });
    let mut client = node_agent_client(address, pki.trusted_files(&pki.controller)).await;
    let call = tokio::spawn(async move { client.describe(Request::new(DescribeRequest {})).await });
    entered.acquire().await.unwrap().forget();
    shutdown.begin_drain();

    let result = timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("deadline exceeded")
    );
    assert!(shutdown.force().is_cancelled());
    assert_eq!(connections.len(), 0);
    assert_eq!(requests.len(), 0);
    assert!(call.await.unwrap().is_err());
    assert!(TcpStream::connect(address).await.is_err());
}

#[tokio::test]
async fn admitted_node_rpc_finishes_during_grace_without_force_cancellation() {
    let pki = TestPki::generate();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let shutdown = AgentShutdown::new("500ms".parse::<ShutdownGrace>().unwrap());
    let connections = TaskTracker::new();
    let requests = TaskTracker::new();
    let service = NodeAgentService::new(
        NODE_ID,
        Arc::new(ReleasableEngine {
            entered: entered.clone(),
            release: release.clone(),
        }),
        shutdown.force(),
    );
    let (failures, task_failures) = tokio::sync::mpsc::unbounded_channel();
    let server = serve_node_agent_rpc(
        listener,
        CLUSTER_ID.to_owned(),
        pki.trusted_files(&pki.agent),
        service,
        shutdown.draining(),
        shutdown.force(),
        connections.clone(),
        requests.clone(),
        failures,
    )
    .unwrap();
    let task_shutdown = shutdown.clone();
    let task_connections = connections.clone();
    let task_requests = requests.clone();
    let task = tokio::spawn(async move {
        supervise_node_agent_server(
            server,
            task_shutdown,
            task_connections,
            task_requests,
            task_failures,
            pending(),
        )
        .await
    });
    let mut client = node_agent_client(address, pki.trusted_files(&pki.controller)).await;
    let call = tokio::spawn(async move { client.describe(Request::new(DescribeRequest {})).await });
    entered.acquire().await.unwrap().forget();
    shutdown.begin_drain();
    release.add_permits(1);

    let response = timeout(Duration::from_secs(2), call)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(response.into_inner().node_id, NODE_ID);
    timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!shutdown.force().is_cancelled());
    assert_eq!(connections.len(), 0);
    assert_eq!(requests.len(), 0);
}

#[tokio::test]
async fn rpc_request_panic_drains_the_node_listener() {
    let pki = TestPki::generate();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let shutdown = AgentShutdown::new("250ms".parse::<ShutdownGrace>().unwrap());
    let connections = TaskTracker::new();
    let requests = TaskTracker::new();
    let (failures, task_failures) = tokio::sync::mpsc::unbounded_channel();
    let service = NodeAgentService::new(NODE_ID, Arc::new(PanicEngine), shutdown.force());
    let server = serve_node_agent_rpc(
        listener,
        CLUSTER_ID.to_owned(),
        pki.trusted_files(&pki.agent),
        service,
        shutdown.draining(),
        shutdown.force(),
        connections.clone(),
        requests.clone(),
        failures,
    )
    .unwrap();
    let task_shutdown = shutdown.clone();
    let task_connections = connections.clone();
    let task_requests = requests.clone();
    let task = tokio::spawn(async move {
        supervise_node_agent_server(
            server,
            task_shutdown,
            task_connections,
            task_requests,
            task_failures,
            pending(),
        )
        .await
    });
    let mut client = node_agent_client(address, pki.trusted_files(&pki.controller)).await;
    let call = tokio::spawn(async move { client.describe(Request::new(DescribeRequest {})).await });

    let result = timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    call.abort();
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("rpc request task failed")
    );
    assert!(shutdown.draining().is_cancelled());
    assert!(!shutdown.force().is_cancelled());
    assert_eq!(connections.len(), 0);
    assert_eq!(requests.len(), 0);
    assert!(TcpStream::connect(address).await.is_err());
}

#[tokio::test]
async fn shared_shutdown_joins_controller_http_and_rpc_tasks() {
    let pki = TestPki::generate();
    let http_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let http_address = http_listener.local_addr().unwrap();
    let rpc_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let rpc_address = rpc_listener.local_addr().unwrap();
    let cancellation = CancellationToken::new();
    let task_cancellation = cancellation.clone();
    let entered = Arc::new(Semaphore::new(0));
    let released = Arc::new(Semaphore::new(0));
    let handler_entered = entered.clone();
    let handler_released = released.clone();
    let router = Router::new().route(
        "/health",
        get(move || async move {
            handler_entered.add_permits(1);
            handler_released.acquire().await.unwrap().forget();
            "ok"
        }),
    );
    let tls_files = pki.trusted_files(&pki.controller);
    let mut task = tokio::spawn(async move {
        supervise_controller_servers(
            router,
            BoundHttpServer {
                listener: http_listener,
                transport: HttpTransport::Plaintext,
            },
            Some(BoundAgentRpcServer {
                listener: rpc_listener,
                cluster_id: CLUSTER_ID.to_owned(),
                tls_files,
                intake: ControllerIntakeService::default(),
            }),
            task_cancellation,
        )
        .await
    });

    let mut http = TcpStream::connect(http_address).await.unwrap();
    http.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let http_call = tokio::spawn(async move {
        let mut response = Vec::new();
        http.read_to_end(&mut response).await.unwrap();
        response
    });
    entered.acquire().await.unwrap().forget();

    let mut intake = controller_client(rpc_address, pki.trusted_files(&pki.agent)).await;
    intake.check_in(check_in_request(NODE_ID)).await.unwrap();

    cancellation.cancel();
    assert!(
        timeout(Duration::from_millis(20), &mut task).await.is_err(),
        "controller supervisor dropped the in-flight HTTP handler instead of draining it"
    );
    released.add_permits(1);
    let response = timeout(Duration::from_secs(2), http_call)
        .await
        .expect("HTTP request did not drain")
        .unwrap();
    assert!(response.starts_with(b"HTTP/1.1 200 OK"));
    timeout(Duration::from_secs(2), task)
        .await
        .expect("controller HTTP/RPC supervisor did not stop")
        .expect("controller HTTP/RPC supervisor task panicked")
        .expect("controller HTTP/RPC supervisor returned an error");
    assert!(
        TcpStream::connect(http_address).await.is_err(),
        "HTTP listener still admitted connections after shutdown"
    );
    assert!(
        TcpStream::connect(rpc_address).await.is_err(),
        "RPC listener still admitted connections after shutdown"
    );
}

#[tokio::test]
async fn runtime_http_task_failure_drains_and_joins_the_rpc_sibling() {
    async fn panic_handler() -> &'static str {
        panic!("handler failed")
    }
    let pki = TestPki::generate();
    let http_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let http_address = http_listener.local_addr().unwrap();
    let rpc_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let rpc_address = rpc_listener.local_addr().unwrap();
    let shutdown = ControllerShutdown::new("250ms".parse::<ShutdownGrace>().unwrap());
    let task_shutdown = shutdown.clone();
    let tls_files = pki.trusted_files(&pki.controller);
    let task = tokio::spawn(async move {
        supervise_controller_servers(
            Router::new().route("/panic", get(panic_handler)),
            BoundHttpServer {
                listener: http_listener,
                transport: HttpTransport::Plaintext,
            },
            Some(BoundAgentRpcServer {
                listener: rpc_listener,
                cluster_id: CLUSTER_ID.to_owned(),
                tls_files,
                intake: ControllerIntakeService::default(),
            }),
            task_shutdown,
        )
        .await
    });
    let mut intake = controller_client(rpc_address, pki.trusted_files(&pki.agent)).await;
    intake.check_in(check_in_request(NODE_ID)).await.unwrap();
    let mut client = TcpStream::connect(http_address).await.unwrap();
    client
        .write_all(b"GET /panic HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();

    let result = timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("public HTTP connection task failed")
    );
    assert!(shutdown.draining().is_cancelled());
    assert!(!shutdown.force().is_cancelled());
    assert!(TcpStream::connect(http_address).await.is_err());
    assert!(TcpStream::connect(rpc_address).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn already_cancelled_controller_shutdown_is_successful() {
    for _ in 0..64 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        timeout(
            Duration::from_secs(2),
            supervise_controller_servers(
                Router::new(),
                BoundHttpServer {
                    listener,
                    transport: HttpTransport::Plaintext,
                },
                None,
                cancellation,
            ),
        )
        .await
        .expect("already cancelled supervisor did not join")
        .expect("intentional cancellation was treated as an unexpected server exit");
    }
}

#[tokio::test]
async fn rpc_startup_failure_cancels_and_joins_the_http_server() {
    let http_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let http_address = http_listener.local_addr().unwrap();
    let rpc_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let rpc_address = rpc_listener.local_addr().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let missing_file = directory.path().join("missing.pem");
    let cancellation = CancellationToken::new();

    let result = timeout(
        Duration::from_secs(2),
        supervise_controller_servers(
            Router::new(),
            BoundHttpServer {
                listener: http_listener,
                transport: HttpTransport::Plaintext,
            },
            Some(BoundAgentRpcServer {
                listener: rpc_listener,
                cluster_id: CLUSTER_ID.to_owned(),
                tls_files: MutualTlsIdentityFiles::new(&missing_file, &missing_file, &missing_file),
                intake: ControllerIntakeService::default(),
            }),
            cancellation.clone(),
        ),
    )
    .await
    .expect("RPC startup failure did not join the HTTP server");

    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("agent RPC server failed")
    );
    assert!(cancellation.is_cancelled());
    assert!(TcpStream::connect(http_address).await.is_err());
    assert!(TcpStream::connect(rpc_address).await.is_err());
}

#[tokio::test]
async fn invalid_tls_fails_before_rpc_polling_or_http_admission() {
    let pki = TestPki::generate();
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.pem");
    let malformed = directory.path().join("malformed.pem");
    std::fs::write(&malformed, "secret-canary-invalid-pem").unwrap();

    for (files, expected_context) in [
        (
            MutualTlsIdentityFiles::new(&missing, &missing, &missing),
            "certificate",
        ),
        (
            MutualTlsIdentityFiles::new(&malformed, &malformed, &malformed),
            "certificate",
        ),
        (pki.mismatched_key_files(), "certificate/private key pair"),
    ] {
        let rpc_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let error = serve_controller_rpc(
            rpc_listener,
            CLUSTER_ID.to_owned(),
            files.clone(),
            ControllerIntakeService::default(),
            CancellationToken::new(),
            CancellationToken::new(),
            TaskTracker::new(),
            TaskTracker::new(),
            unused_failure_sender(),
        )
        .err()
        .expect("controller TLS failure was deferred until polling");
        assert!(!error.to_string().contains("secret-canary"));
        assert!(error.to_string().contains(expected_context));

        let node_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let cancellation = CancellationToken::new();
        let service =
            NodeAgentService::new(NODE_ID, Arc::new(AvailableEngine), cancellation.clone());
        let error = serve_node_agent_rpc(
            node_listener,
            CLUSTER_ID.to_owned(),
            files.clone(),
            service,
            cancellation,
            CancellationToken::new(),
            TaskTracker::new(),
            TaskTracker::new(),
            unused_failure_sender(),
        )
        .err()
        .expect("node-agent TLS failure was deferred until polling");
        assert!(!error.to_string().contains("secret-canary"));
        assert!(error.to_string().contains(expected_context));

        let http_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let http_address = http_listener.local_addr().unwrap();
        let mut queued_request = TcpStream::connect(http_address).await.unwrap();
        queued_request
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let admitted = Arc::new(AtomicUsize::new(0));
        let handler_admitted = admitted.clone();
        let router = Router::new().route(
            "/",
            get(move || {
                let admitted = handler_admitted.clone();
                async move {
                    admitted.fetch_add(1, Ordering::SeqCst);
                    "should not run"
                }
            }),
        );
        let rpc_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let rpc_address = rpc_listener.local_addr().unwrap();
        let cancellation = CancellationToken::new();
        let result = timeout(
            Duration::from_secs(2),
            supervise_controller_servers(
                router,
                BoundHttpServer {
                    listener: http_listener,
                    transport: HttpTransport::Plaintext,
                },
                Some(BoundAgentRpcServer {
                    listener: rpc_listener,
                    cluster_id: CLUSTER_ID.to_owned(),
                    tls_files: files,
                    intake: ControllerIntakeService::default(),
                }),
                cancellation.clone(),
            ),
        )
        .await
        .unwrap();

        assert!(result.is_err());
        assert!(cancellation.is_cancelled());
        assert_eq!(admitted.load(Ordering::SeqCst), 0);
        assert!(TcpStream::connect(http_address).await.is_err());
        assert!(TcpStream::connect(rpc_address).await.is_err());
    }
}

async fn start_controller(pki: &TestPki) -> RunningServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let cancellation = CancellationToken::new();
    let task_cancellation = cancellation.clone();
    let tls_files = pki.trusted_files(&pki.controller);
    let task = tokio::spawn(async move {
        serve_controller_rpc(
            listener,
            CLUSTER_ID.to_owned(),
            tls_files,
            ControllerIntakeService::default(),
            task_cancellation.clone(),
            task_cancellation,
            TaskTracker::new(),
            TaskTracker::new(),
            unused_failure_sender(),
        )?
        .await
        .map_err(Into::into)
    });

    RunningServer {
        address,
        cancellation,
        task,
    }
}

async fn start_node_agent(pki: &TestPki, engine: Arc<dyn ContainerEngine>) -> RunningServer {
    start_node_agent_with_tls(pki, pki.trusted_files(&pki.agent), NODE_ID, engine).await
}

async fn start_node_agent_with_tls(
    _pki: &TestPki,
    tls_files: MutualTlsIdentityFiles,
    service_node_id: &str,
    engine: Arc<dyn ContainerEngine>,
) -> RunningServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let cancellation = CancellationToken::new();
    let service = NodeAgentService::new(service_node_id, engine, cancellation.clone());
    let task_cancellation = cancellation.clone();
    let task = tokio::spawn(async move {
        serve_node_agent_rpc(
            listener,
            CLUSTER_ID.to_owned(),
            tls_files,
            service,
            task_cancellation.clone(),
            task_cancellation,
            TaskTracker::new(),
            TaskTracker::new(),
            unused_failure_sender(),
        )?
        .await
        .map_err(Into::into)
    });

    RunningServer {
        address,
        cancellation,
        task,
    }
}

fn production_client(
    address: std::net::SocketAddr,
    tls_files: MutualTlsIdentityFiles,
    server_name: &str,
    cluster_id: &str,
    node_id: &str,
) -> NodeAgentClientConfig {
    NodeAgentClientConfig::new(
        format!("https://{address}"),
        server_name.to_owned(),
        cluster_id.to_owned(),
        node_id.to_owned(),
        tls_files,
    )
    .expect("production client fixture must validate")
}

async fn controller_client(
    address: std::net::SocketAddr,
    identity: MutualTlsIdentityFiles,
) -> ControllerIntakeClient<Channel> {
    let channel = connect(address, identity.client_config(CONTROLLER_NAME).unwrap())
        .await
        .unwrap();
    ControllerIntakeClient::new(channel)
}

async fn node_agent_client(
    address: std::net::SocketAddr,
    identity: MutualTlsIdentityFiles,
) -> NodeAgentClient<Channel> {
    let channel = connect(address, identity.client_config(NODE_AGENT_NAME).unwrap())
        .await
        .unwrap();
    NodeAgentClient::new(channel)
}

async fn connect(
    address: std::net::SocketAddr,
    tls_config: ClientTlsConfig,
) -> Result<Channel, tonic::transport::Error> {
    Endpoint::from_shared(format!("https://{address}"))?
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(2))
        .tls_config(tls_config)?
        .connect()
        .await
}

async fn assert_controller_exchange_rejected(
    address: std::net::SocketAddr,
    tls_config: ClientTlsConfig,
    fixture: &str,
) {
    let rejected = match connect(address, tls_config).await {
        Ok(channel) => ControllerIntakeClient::new(channel)
            .check_in(check_in_request(NODE_ID))
            .await
            .is_err(),
        Err(_) => true,
    };
    assert!(rejected, "{fixture} unexpectedly completed an RPC");
}

async fn assert_controller_plaintext_rejected(address: std::net::SocketAddr) {
    let channel = Endpoint::from_shared(format!("http://{address}"))
        .unwrap()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(2))
        .connect()
        .await;
    let rejected = match channel {
        Ok(channel) => ControllerIntakeClient::new(channel)
            .check_in(check_in_request(NODE_ID))
            .await
            .is_err(),
        Err(_) => true,
    };
    assert!(rejected, "plaintext unexpectedly completed an RPC");
}

fn check_in_request(agent_id: &str) -> CheckInRequest {
    CheckInRequest {
        agent_id: agent_id.to_owned(),
        agent_version: "0.1.0-test".to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
        ..CheckInRequest::default()
    }
}
