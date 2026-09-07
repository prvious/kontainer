use std::sync::Arc;

use kontainer_agent_proto::v1::{DescribeRequest, DescribeResponse, node_agent_server::NodeAgent};
use kontainer_engine::ContainerEngine;
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
}

#[cfg(test)]
mod tests {
    use super::{ContainerEngine, NodeAgent, NodeAgentService};
    use async_trait::async_trait;
    use kontainer_agent_proto::v1::DescribeRequest;
    use kontainer_engine::EngineError;
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;
    use tonic::Request;

    struct AvailableEngine;

    #[async_trait]
    impl ContainerEngine for AvailableEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }
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
}
