mod support;

use std::{future::pending, sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::{Router, routing::get};
use kontainer_agent_proto::{
    PROTOCOL_PACKAGE,
    v1::{
        CheckInRequest, DescribeRequest, controller_intake_client::ControllerIntakeClient,
        node_agent_client::NodeAgentClient,
    },
};
use kontainer_controller::rpc::serve_controller_rpc;
use kontainer_controller::server::{BoundAgentRpcServer, supervise_controller_servers};
use kontainer_engine::{ContainerEngine, EngineError};
use kontainer_node_agent::{NodeAgentService, serve_node_agent_rpc};
use kontainer_rpc_auth::MutualTlsIdentityFiles;
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
use tokio_util::sync::CancellationToken;
use tonic::{
    Code, Request,
    transport::{Channel, ClientTlsConfig, Endpoint},
};

type ServerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

struct AvailableEngine;

#[async_trait]
impl ContainerEngine for AvailableEngine {
    async fn is_available(&self) -> Result<bool, EngineError> {
        Ok(true)
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
    assert!(matches!(error.code(), Code::Cancelled | Code::Unavailable));
    timeout(Duration::from_secs(2), node_agent.task)
        .await
        .expect("node-agent server did not join after cancellation")
        .expect("node-agent server task panicked")
        .expect("node-agent server returned an error");
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
            http_listener,
            Some(BoundAgentRpcServer {
                listener: rpc_listener,
                cluster_id: CLUSTER_ID.to_owned(),
                tls_files,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn already_cancelled_controller_shutdown_is_successful() {
    for _ in 0..64 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        timeout(
            Duration::from_secs(2),
            supervise_controller_servers(Router::new(), listener, None, cancellation),
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
            http_listener,
            Some(BoundAgentRpcServer {
                listener: rpc_listener,
                cluster_id: CLUSTER_ID.to_owned(),
                tls_files: MutualTlsIdentityFiles::new(&missing_file, &missing_file, &missing_file),
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
            task_cancellation,
        )
        .await
    });

    RunningServer {
        address,
        cancellation,
        task,
    }
}

async fn start_node_agent(pki: &TestPki, engine: Arc<dyn ContainerEngine>) -> RunningServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let cancellation = CancellationToken::new();
    let service = NodeAgentService::new(NODE_ID, engine, cancellation.clone());
    let task_cancellation = cancellation.clone();
    let tls_files = pki.trusted_files(&pki.agent);
    let task = tokio::spawn(async move {
        serve_node_agent_rpc(
            listener,
            CLUSTER_ID.to_owned(),
            tls_files,
            service,
            task_cancellation,
        )
        .await
    });

    RunningServer {
        address,
        cancellation,
        task,
    }
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
    }
}
