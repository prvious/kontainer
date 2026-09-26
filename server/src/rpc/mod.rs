pub mod client;

pub use client::{
    AgentClient, AgentClientConfigError, AgentClientError, AgentContainerPage, AgentContainerState,
    AgentContainerSummary, AgentDescription, NodeAgentClientConfig, SharedAgentClient,
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
use std::{error::Error, future::Future};
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tonic::service::{Interceptor, Routes};
use tonic::{Request, Response, Status};

#[derive(Debug, Default)]
pub struct ControllerIntakeService;

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

        Ok(Response::new(CheckInResponse {
            controller_version: env!("CARGO_PKG_VERSION").to_owned(),
            protocol_package: PROTOCOL_PACKAGE.to_owned(),
        }))
    }
}

/// Validate TLS eagerly; polling the returned future begins RPC admission.
#[allow(clippy::too_many_arguments)] // Explicit lifecycle and task ownership.
pub fn serve_controller_rpc(
    listener: TcpListener,
    cluster_id: String,
    tls_files: MutualTlsIdentityFiles,
    cancellation: CancellationToken,
    force: CancellationToken,
    connections: TaskTracker,
    request_tasks: TaskTracker,
    failures: UnboundedSender<&'static str>,
) -> Result<impl Future<Output = std::io::Result<()>> + Send, Box<dyn Error + Send + Sync>> {
    let mut peer_identity = PeerIdentityInterceptor::new(cluster_id, WorkloadKind::NodeAgent)?;
    let acceptor = tls_files.server_acceptor()?;
    let admission = cancellation.clone();
    let service =
        ControllerIntakeServer::with_interceptor(ControllerIntakeService, move |request| {
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
    use super::{CheckInRequest, ControllerIntake, ControllerIntakeService, Request};
    use kontainer_agent_proto::PROTOCOL_PACKAGE;

    #[tokio::test]
    async fn check_in_does_not_trust_an_unverified_protobuf_identity() {
        let error = ControllerIntakeService
            .check_in(Request::new(CheckInRequest {
                agent_id: "node-1".to_owned(),
                agent_version: "0.1.0".to_owned(),
                architecture: "aarch64".to_owned(),
            }))
            .await
            .unwrap_err();

        assert_eq!(error.code(), tonic::Code::Unauthenticated);
        assert_eq!(PROTOCOL_PACKAGE, "kontainer.agent.v1");
    }

    #[tokio::test]
    async fn check_in_requires_an_authenticated_peer_before_validating_claims() {
        let error = ControllerIntakeService
            .check_in(Request::new(CheckInRequest::default()))
            .await
            .unwrap_err();

        assert_eq!(error.code(), tonic::Code::Unauthenticated);
    }
}
