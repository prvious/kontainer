pub mod client;

pub use client::{
    AgentClient, AgentClientConfigError, AgentClientError, AgentContainerAction,
    AgentContainerPage, AgentContainerState, AgentContainerSummary, AgentDescription, AgentImage,
    AgentPullEvent, AgentPullStream, NodeAgentClientConfig,
};
use kontainer_agent_proto::{
    PROTOCOL_PACKAGE,
    v1::{
        CheckInRequest, CheckInResponse,
        controller_intake_server::{ControllerIntake, ControllerIntakeServer},
    },
};
use kontainer_rpc_auth::{
    AuthenticatedPeerIdentity, MutualTlsIdentityFiles, PeerIdentityInterceptor, WorkloadKind,
};
use sea_orm::DatabaseConnection;
use std::{error::Error, future::Future, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tonic::service::{Interceptor, Routes};
use tonic::{Request, Response, Status};

use crate::nodes::{
    actions::record_check_in::{self, RecordCheckInError},
    domain::{CHECK_IN_INTERVAL_SECONDS, CheckIn, SampleReading},
};

/// Where verified check-ins go. Production writes PostgreSQL; transport tests record nothing.
#[tonic::async_trait]
pub trait CheckInRecorder: Send + Sync {
    async fn record(&self, check_in: CheckIn) -> Result<(), RecordCheckInError>;
}

pub struct PostgresCheckInRecorder {
    database: DatabaseConnection,
    cluster_id: String,
}

impl PostgresCheckInRecorder {
    #[must_use]
    pub fn new(database: DatabaseConnection, cluster_id: String) -> Self {
        Self {
            database,
            cluster_id,
        }
    }
}

#[tonic::async_trait]
impl CheckInRecorder for PostgresCheckInRecorder {
    async fn record(&self, check_in: CheckIn) -> Result<(), RecordCheckInError> {
        record_check_in::execute(&self.database, &self.cluster_id, check_in).await
    }
}

/// `ControllerIntake`: verifies the agent claim against its certificate, then records it.
#[derive(Clone, Default)]
pub struct ControllerIntakeService {
    recorder: Option<Arc<dyn CheckInRecorder>>,
}

impl ControllerIntakeService {
    #[must_use]
    pub fn recording(recorder: Arc<dyn CheckInRecorder>) -> Self {
        Self {
            recorder: Some(recorder),
        }
    }
}

/// Map the wire message onto the domain check-in; the node id is the certificate's.
fn domain_check_in(node_id: String, check_in: CheckInRequest) -> Result<CheckIn, Status> {
    let timestamp = |unix_ms: i64| {
        chrono::DateTime::from_timestamp_millis(unix_ms)
            .ok_or_else(|| Status::invalid_argument("a timestamp is out of range"))
    };
    let sample = check_in
        .sample
        .map(|sample| {
            Ok::<_, Status>(SampleReading {
                sampled_at: timestamp(sample.sampled_at_unix_ms)?,
                #[allow(clippy::cast_possible_truncation)]
                cpu_percent: sample.cpu_percent as f32,
                memory_used_bytes: sample.memory_used_bytes,
                disk_used_bytes: sample.disk_used_bytes,
                containers_running: sample.containers_running,
                containers_paused: sample.containers_paused,
                containers_exited: sample.containers_exited,
                net_rx_bytes_per_sec: sample.net_rx_bytes_per_sec,
                net_tx_bytes_per_sec: sample.net_tx_bytes_per_sec,
            })
        })
        .transpose()?;
    Ok(CheckIn {
        node_id,
        agent_version: check_in.agent_version,
        architecture: check_in.architecture,
        advertise_address: check_in.advertise_address,
        hostname: check_in.hostname,
        swarm_node_id: Some(check_in.swarm_node_id).filter(|id| !id.is_empty()),
        docker_version: check_in.docker_version,
        os: check_in.os,
        cpu_cores: check_in.cpu_cores,
        memory_total_bytes: check_in.memory_total_bytes,
        agent_started_at: timestamp(check_in.agent_started_at_unix_ms)?,
        sample,
    })
}

#[tonic::async_trait]
impl ControllerIntake for ControllerIntakeService {
    async fn check_in(
        &self,
        request: Request<CheckInRequest>,
    ) -> Result<Response<CheckInResponse>, Status> {
        let authenticated_node_id = request
            .extensions()
            .get::<AuthenticatedPeerIdentity>()
            .and_then(|identity| identity.workload().node_id())
            .ok_or_else(|| Status::unauthenticated("authenticated node identity is required"))?
            .to_owned();
        let check_in = request.into_inner();
        if check_in.agent_id.trim().is_empty() {
            return Err(Status::invalid_argument("agent_id is required"));
        }
        if check_in.agent_id != authenticated_node_id {
            return Err(Status::permission_denied(
                "agent claim does not match its certificate identity",
            ));
        }
        if let Some(recorder) = &self.recorder {
            let check_in = domain_check_in(authenticated_node_id, check_in)?;
            recorder
                .record(check_in)
                .await
                .map_err(|error| match error {
                    RecordCheckInError::Rejected(rejection) => {
                        Status::invalid_argument(rejection.to_string())
                    }
                    RecordCheckInError::Persistence(error) => {
                        tracing::warn!(%error, "check-in persistence failed");
                        Status::unavailable("controller persistence is unavailable")
                    }
                })?;
        }

        Ok(Response::new(CheckInResponse {
            controller_version: env!("CARGO_PKG_VERSION").to_owned(),
            protocol_package: PROTOCOL_PACKAGE.to_owned(),
            check_in_interval_seconds: CHECK_IN_INTERVAL_SECONDS,
        }))
    }
}

/// Validate TLS eagerly; polling the returned future begins RPC admission.
#[allow(clippy::too_many_arguments)] // Explicit lifecycle and task ownership.
pub fn serve_controller_rpc(
    listener: TcpListener,
    cluster_id: String,
    tls_files: MutualTlsIdentityFiles,
    intake: ControllerIntakeService,
    cancellation: CancellationToken,
    force: CancellationToken,
    connections: TaskTracker,
    request_tasks: TaskTracker,
    failures: UnboundedSender<&'static str>,
) -> Result<impl Future<Output = std::io::Result<()>> + Send, Box<dyn Error + Send + Sync>> {
    let mut peer_identity = PeerIdentityInterceptor::new(cluster_id, WorkloadKind::NodeAgent)?;
    let acceptor = tls_files.server_acceptor()?;
    let admission = cancellation.clone();
    let service = ControllerIntakeServer::with_interceptor(intake, move |request| {
        let request = peer_identity.call(request)?;
        if admission.is_cancelled() {
            Err(Status::unavailable("controller is draining"))
        } else {
            Ok(request)
        }
    });
    Ok(kontainer_rpc_auth::serve_rpc_listener(
        listener,
        acceptor,
        Routes::new(service).prepare(),
        cancellation,
        force,
        connections,
        request_tasks,
        failures,
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        CheckIn, CheckInRecorder, CheckInRequest, ControllerIntake, ControllerIntakeService,
        RecordCheckInError, Request,
    };
    use kontainer_agent_proto::{PROTOCOL_PACKAGE, v1::NodeSample};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct CapturingRecorder(Mutex<Vec<CheckIn>>);

    #[tonic::async_trait]
    impl CheckInRecorder for CapturingRecorder {
        async fn record(&self, check_in: CheckIn) -> Result<(), RecordCheckInError> {
            crate::nodes::domain::validate(&check_in)?;
            self.0.lock().unwrap().push(check_in);
            Ok(())
        }
    }

    fn authenticated(request: CheckInRequest) -> Request<CheckInRequest> {
        let mut request = Request::new(request);
        request
            .extensions_mut()
            .insert(kontainer_rpc_auth::AuthenticatedPeerIdentity::agent_for_tests("manager"));
        request
    }

    fn full_check_in(agent_id: &str) -> CheckInRequest {
        CheckInRequest {
            agent_id: agent_id.to_owned(),
            agent_version: "0.1.0".to_owned(),
            architecture: "aarch64".to_owned(),
            advertise_address: "127.0.0.1:50052".to_owned(),
            hostname: "manager".to_owned(),
            swarm_node_id: String::new(),
            docker_version: "29.8.0".to_owned(),
            os: "Alpine Linux".to_owned(),
            cpu_cores: 4,
            memory_total_bytes: 8 << 30,
            agent_started_at_unix_ms: 1_760_000_000_000,
            sample: Some(NodeSample {
                sampled_at_unix_ms: 1_760_000_005_000,
                cpu_percent: 12.5,
                memory_used_bytes: 1 << 30,
                disk_used_bytes: None,
                containers_running: 3,
                containers_paused: 0,
                containers_exited: 1,
                net_rx_bytes_per_sec: None,
                net_tx_bytes_per_sec: None,
            }),
        }
    }

    #[tokio::test]
    async fn verified_check_ins_are_recorded_under_the_certificate_node_id() {
        let recorder = Arc::new(CapturingRecorder::default());
        let service = ControllerIntakeService::recording(recorder.clone());
        let node = "manager".to_owned();

        let response = service
            .check_in(authenticated(full_check_in(&node)))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(response.check_in_interval_seconds, 5);
        let recorded = recorder.0.lock().unwrap().pop().unwrap();
        assert_eq!(recorded.node_id, node);
        assert_eq!(recorded.swarm_node_id, None, "empty means not in a Swarm");
        assert_eq!(recorded.sample.unwrap().cpu_percent, 12.5);

        // An agent that sends no sample still checks in.
        let mut without_sample = full_check_in(&node);
        without_sample.sample = None;
        service
            .check_in(authenticated(without_sample))
            .await
            .unwrap();
        assert!(recorder.0.lock().unwrap().pop().unwrap().sample.is_none());

        let mut bad = full_check_in(&node);
        bad.advertise_address = "nowhere".to_owned();
        let error = service.check_in(authenticated(bad)).await.unwrap_err();
        assert_eq!(error.code(), tonic::Code::InvalidArgument);
        assert!(recorder.0.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn check_in_does_not_trust_an_unverified_protobuf_identity() {
        let error = ControllerIntakeService::default()
            .check_in(Request::new(CheckInRequest {
                agent_id: "node-1".to_owned(),
                agent_version: "0.1.0".to_owned(),
                architecture: "aarch64".to_owned(),
                ..CheckInRequest::default()
            }))
            .await
            .unwrap_err();

        assert_eq!(error.code(), tonic::Code::Unauthenticated);
        assert_eq!(PROTOCOL_PACKAGE, "kontainer.agent.v1");
    }

    #[tokio::test]
    async fn check_in_requires_an_authenticated_peer_before_validating_claims() {
        let error = ControllerIntakeService::default()
            .check_in(Request::new(CheckInRequest::default()))
            .await
            .unwrap_err();

        assert_eq!(error.code(), tonic::Code::Unauthenticated);
    }
}
