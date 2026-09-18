use std::sync::Arc;

use kontainer_agent_proto::v1::{
    ContainerState as ProtoContainerState, ContainerSummary as ProtoContainerSummary,
    DescribeRequest, DescribeResponse, ListContainersRequest, ListContainersResponse,
    node_agent_server::NodeAgent,
};
use kontainer_engine::{
    ContainerEngine, ContainerPageRequest, ContainerState, EngineError, EngineErrorKind,
};
use kontainer_rpc_auth::AuthenticatedPeerIdentity;
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status};
use tracing::warn;

#[derive(Clone)]
pub struct NodeAgentService {
    node_id: String,
    engine: Arc<dyn ContainerEngine>,
    cancellation: CancellationToken,
}

impl NodeAgentService {
    #[must_use]
    pub fn new(
        node_id: impl Into<String>,
        engine: Arc<dyn ContainerEngine>,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            engine,
            cancellation,
        }
    }
}

#[tonic::async_trait]
impl NodeAgent for NodeAgentService {
    async fn describe(
        &self,
        request: Request<DescribeRequest>,
    ) -> Result<Response<DescribeResponse>, Status> {
        request
            .extensions()
            .get::<AuthenticatedPeerIdentity>()
            .ok_or_else(|| {
                Status::unauthenticated("authenticated controller identity is required")
            })?;

        let docker_connected = tokio::select! {
            _ = self.cancellation.cancelled() => {
                return Err(Status::cancelled("node agent is shutting down"));
            }
            availability = self.engine.is_available() => match availability {
                Ok(is_available) => is_available,
                Err(error) => {
                    warn!(%error, "Docker health check failed");
                    false
                }
            }
        };

        Ok(Response::new(DescribeResponse {
            node_id: self.node_id.clone(),
            agent_version: env!("CARGO_PKG_VERSION").to_owned(),
            architecture: std::env::consts::ARCH.to_owned(),
            docker_connected,
        }))
    }

    async fn list_containers(
        &self,
        request: Request<ListContainersRequest>,
    ) -> Result<Response<ListContainersResponse>, Status> {
        request
            .extensions()
            .get::<AuthenticatedPeerIdentity>()
            .ok_or_else(|| {
                Status::unauthenticated("authenticated controller identity is required")
            })?;

        let inner = request.into_inner();
        let page_request = ContainerPageRequest::new(inner.page_size, &inner.page_token)
            .map_err(|_| Status::invalid_argument("pagination is invalid"))?;

        let page = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => {
                return Err(Status::cancelled("node agent is shutting down"));
            }
            page = self.engine.list_containers(page_request, &self.cancellation) => {
                page.map_err(|error| {
                    // Stable kinds only; daemon detail stays in the error value,
                    // never in the message.
                    warn!(%error, "container inventory failed");
                    engine_error_to_status(&error)
                })?
            }
        };

        let node_id = self.node_id.clone();
        Ok(Response::new(ListContainersResponse {
            containers: page
                .containers
                .into_iter()
                .map(|container| ProtoContainerSummary {
                    container_id: container.container_id,
                    display_name: container.display_name,
                    image_reference: container.image_reference,
                    state: domain_state_to_proto(container.state) as i32,
                    status: container.status,
                    created_at_unix_seconds: container.created_at_seconds,
                    node_id: node_id.clone(),
                })
                .collect(),
            next_page_token: page.next_page_token.unwrap_or_default(),
        }))
    }
}

fn engine_error_to_status(error: &EngineError) -> Status {
    match error.kind() {
        EngineErrorKind::InvalidInput => Status::invalid_argument("pagination is invalid"),
        EngineErrorKind::Cancelled => Status::cancelled("container listing was cancelled"),
        EngineErrorKind::DeadlineExceeded => Status::deadline_exceeded("engine timed out"),
        EngineErrorKind::ConnectionUnavailable
        | EngineErrorKind::DaemonRejected
        | EngineErrorKind::DependencyFailure => Status::unavailable("engine is unavailable"),
        EngineErrorKind::UnsupportedCapability | EngineErrorKind::UnsupportedApiVersion => {
            Status::failed_precondition("container inventory is not supported")
        }
        EngineErrorKind::InvalidResponse | EngineErrorKind::InvalidConfiguration => {
            Status::internal("engine response was invalid")
        }
    }
}

fn domain_state_to_proto(state: ContainerState) -> ProtoContainerState {
    match state {
        ContainerState::Unknown => ProtoContainerState::Unknown,
        ContainerState::Created => ProtoContainerState::Created,
        ContainerState::Running => ProtoContainerState::Running,
        ContainerState::Paused => ProtoContainerState::Paused,
        ContainerState::Restarting => ProtoContainerState::Restarting,
        ContainerState::Removing => ProtoContainerState::Removing,
        ContainerState::Exited => ProtoContainerState::Exited,
        ContainerState::Dead => ProtoContainerState::Dead,
        ContainerState::Stopping => ProtoContainerState::Stopping,
    }
}

#[cfg(test)]
mod tests {
    use super::{ContainerEngine, NodeAgent, NodeAgentService};
    use async_trait::async_trait;
    use kontainer_agent_proto::v1::{
        ContainerState as ProtoContainerState, DescribeRequest, ListContainersRequest,
    };
    use kontainer_engine::{
        ContainerPage, ContainerPageRequest, ContainerState, ContainerSummary, EngineError,
        EngineErrorKind, EngineOperation,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio_util::sync::CancellationToken;
    use tonic::Request;

    struct AvailableEngine;

    #[async_trait]
    impl ContainerEngine for AvailableEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }
    }

    #[derive(Debug, Clone)]
    struct ScriptedEngine {
        pages: Vec<ContainerPage>,
        calls: Arc<AtomicUsize>,
        error: Option<(EngineErrorKind, EngineOperation)>,
    }

    #[async_trait]
    impl ContainerEngine for ScriptedEngine {
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

    struct BlockingEngine;

    #[async_trait]
    impl ContainerEngine for BlockingEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }

        async fn list_containers(
            &self,
            _request: ContainerPageRequest,
            cancellation: &CancellationToken,
        ) -> Result<ContainerPage, EngineError> {
            cancellation.cancelled().await;
            Err(EngineError::new(
                EngineErrorKind::Cancelled,
                EngineOperation::ListContainers,
            ))
        }
    }

    fn summary(id: &str, name: &str) -> ContainerSummary {
        ContainerSummary {
            container_id: id.to_owned(),
            display_name: name.to_owned(),
            image_reference: "nginx:latest".to_owned(),
            state: ContainerState::Running,
            status: "Up 3s".to_owned(),
            created_at_seconds: 1_700_000_000,
        }
    }

    fn authenticated<Service>(request: Service) -> Request<Service> {
        let mut request = Request::new(request);
        request
            .extensions_mut()
            .insert(kontainer_rpc_auth::AuthenticatedPeerIdentity::new_for_tests());
        request
    }

    #[tokio::test]
    async fn describe_does_not_run_for_an_unauthenticated_caller() {
        let service = NodeAgentService::new(
            "node-1",
            Arc::new(AvailableEngine),
            CancellationToken::new(),
        );

        let error = service
            .describe(Request::new(DescribeRequest {}))
            .await
            .unwrap_err();

        assert_eq!(error.code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn list_containers_rejects_unauthenticated_callers_before_validation() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = NodeAgentService::new(
            "node-1",
            Arc::new(ScriptedEngine {
                pages: vec![ContainerPage {
                    containers: vec![],
                    next_page_token: None,
                }],
                calls: calls.clone(),
                error: None,
            }),
            CancellationToken::new(),
        );

        let error = service
            .list_containers(Request::new(ListContainersRequest {
                page_size: 0,
                page_token: String::new(),
            }))
            .await
            .unwrap_err();

        assert_eq!(error.code(), tonic::Code::Unauthenticated);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn list_containers_validates_pagination_without_engine_access() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = NodeAgentService::new(
            "node-1",
            Arc::new(ScriptedEngine {
                pages: vec![ContainerPage {
                    containers: vec![],
                    next_page_token: None,
                }],
                calls: calls.clone(),
                error: None,
            }),
            CancellationToken::new(),
        );

        for request in [
            ListContainersRequest {
                page_size: 101,
                page_token: String::new(),
            },
            ListContainersRequest {
                page_size: 10,
                page_token: "not-a-cursor".to_owned(),
            },
            ListContainersRequest {
                page_size: 10,
                page_token: "v1:short".to_owned(),
            },
        ] {
            let error = service
                .list_containers(authenticated(request))
                .await
                .unwrap_err();
            assert_eq!(error.code(), tonic::Code::InvalidArgument);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn list_containers_returns_configured_provenance_and_cursors() {
        let first_id = "a".repeat(64);
        let second_id = "b".repeat(64);
        let cursor = format!("v1:{first_id}");
        let calls = Arc::new(AtomicUsize::new(0));
        let service = NodeAgentService::new(
            "node-9",
            Arc::new(ScriptedEngine {
                pages: vec![
                    ContainerPage {
                        containers: vec![summary(&first_id, "web")],
                        next_page_token: Some(cursor.clone()),
                    },
                    ContainerPage {
                        containers: vec![summary(&second_id, "worker")],
                        next_page_token: None,
                    },
                ],
                calls: calls.clone(),
                error: None,
            }),
            CancellationToken::new(),
        );

        let first = service
            .list_containers(authenticated(ListContainersRequest {
                page_size: 1,
                page_token: String::new(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(first.containers.len(), 1);
        assert_eq!(first.containers[0].container_id, first_id);
        assert_eq!(first.containers[0].node_id, "node-9");
        assert_eq!(
            first.containers[0].state,
            ProtoContainerState::Running as i32
        );
        assert_eq!(first.next_page_token, cursor);

        let second = service
            .list_containers(authenticated(ListContainersRequest {
                page_size: 1,
                page_token: cursor,
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(second.containers.len(), 1);
        assert_eq!(second.containers[0].container_id, second_id);
        assert_eq!(second.containers[0].node_id, "node-9");
        assert!(second.next_page_token.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn list_containers_maps_engine_failures_to_stable_statuses() {
        for (kind, code) in [
            (
                EngineErrorKind::DeadlineExceeded,
                tonic::Code::DeadlineExceeded,
            ),
            (
                EngineErrorKind::ConnectionUnavailable,
                tonic::Code::Unavailable,
            ),
            (EngineErrorKind::DaemonRejected, tonic::Code::Unavailable),
            (
                EngineErrorKind::UnsupportedCapability,
                tonic::Code::FailedPrecondition,
            ),
            (EngineErrorKind::InvalidResponse, tonic::Code::Internal),
        ] {
            let service = NodeAgentService::new(
                "node-1",
                Arc::new(ScriptedEngine {
                    pages: vec![],
                    calls: Arc::new(AtomicUsize::new(0)),
                    error: Some((kind, EngineOperation::ListContainers)),
                }),
                CancellationToken::new(),
            );
            let error = service
                .list_containers(authenticated(ListContainersRequest {
                    page_size: 10,
                    page_token: String::new(),
                }))
                .await
                .unwrap_err();
            assert_eq!(error.code(), code, "{kind:?}");
            assert!(!error.message().contains("secret-canary"));
        }
    }

    #[tokio::test]
    async fn list_containers_cancellation_wins_without_detached_work() {
        let service =
            NodeAgentService::new("node-1", Arc::new(BlockingEngine), CancellationToken::new());
        let cancellation = service.cancellation.clone();
        cancellation.cancel();

        let error = service
            .list_containers(authenticated(ListContainersRequest {
                page_size: 10,
                page_token: String::new(),
            }))
            .await
            .unwrap_err();
        assert_eq!(error.code(), tonic::Code::Cancelled);
    }
}
