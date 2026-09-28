//! `ManagerEngine`: the controller-owned port for swarm-wide objects (networks now, services in
//! spec 11). The Docker implementation wraps the engine crate's manager-scoped engine built from
//! `settings.docker.host`; a host that is not a Swarm manager leaves the port absent and
//! manager-scoped routes answer 503 `not_a_manager`.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use kontainer_engine::{
    CapabilityStatus, ContainerEngine, DockerHost, EngineErrorKind, LocalDockerEngine,
    LocalEngineOptions, NetworkCreate, NetworkDetail, NetworkSummary,
};
use tokio_util::sync::CancellationToken;

/// Why a manager call failed, without daemon detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagerError {
    /// The daemon could not be reached or timed out.
    Unavailable,
    NotFound,
    /// The request was refused before Docker was asked (e.g. a built-in network).
    InvalidInput,
    /// Docker refused the change (e.g. endpoints are still attached).
    Rejected,
    Cancelled,
}

impl std::fmt::Display for ManagerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "manager engine is unavailable",
            Self::NotFound => "object not found on the manager",
            Self::InvalidInput => "request refused",
            Self::Rejected => "the manager refused the change",
            Self::Cancelled => "manager call was cancelled",
        })
    }
}

impl From<kontainer_engine::EngineError> for ManagerError {
    fn from(error: kontainer_engine::EngineError) -> Self {
        match error.kind() {
            EngineErrorKind::NotFound => Self::NotFound,
            EngineErrorKind::InvalidInput => Self::InvalidInput,
            EngineErrorKind::Cancelled => Self::Cancelled,
            EngineErrorKind::Conflict
            | EngineErrorKind::DaemonRejected
            | EngineErrorKind::NotModified => Self::Rejected,
            _ => Self::Unavailable,
        }
    }
}

#[async_trait]
pub trait ManagerEngine: Send + Sync {
    /// Every network the manager knows, including every swarm-scope network.
    async fn list_networks(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<NetworkSummary>, ManagerError>;

    /// One network with the manager's own endpoints on it.
    async fn inspect_network(
        &self,
        id: &str,
        cancellation: &CancellationToken,
    ) -> Result<NetworkDetail, ManagerError>;

    /// Create a swarm-scope network and return its id.
    async fn create_network(
        &self,
        network: &NetworkCreate,
        cancellation: &CancellationToken,
    ) -> Result<String, ManagerError>;

    async fn remove_network(
        &self,
        id: &str,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagerError>;
}

/// The manager engine routes and executors use, when the configured host is a manager.
#[derive(Clone)]
pub struct SharedManagerEngine(pub Arc<dyn ManagerEngine>);

/// The Docker manager, through the engine crate.
pub struct DockerManagerEngine(LocalDockerEngine);

#[async_trait]
impl ManagerEngine for DockerManagerEngine {
    async fn list_networks(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<NetworkSummary>, ManagerError> {
        Ok(self.0.list_networks(cancellation).await?)
    }

    async fn inspect_network(
        &self,
        id: &str,
        cancellation: &CancellationToken,
    ) -> Result<NetworkDetail, ManagerError> {
        Ok(self.0.inspect_network(id, cancellation).await?)
    }

    async fn create_network(
        &self,
        network: &NetworkCreate,
        cancellation: &CancellationToken,
    ) -> Result<String, ManagerError> {
        Ok(self.0.create_network(network, cancellation).await?)
    }

    async fn remove_network(
        &self,
        id: &str,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagerError> {
        Ok(self.0.remove_network(id, cancellation).await?)
    }
}

const DESCRIBE_DEADLINE: Duration = Duration::from_secs(5);

/// Connect to `host` and keep it only if it is a Swarm manager. Anything else is logged once and
/// yields `None`.
pub async fn connect_manager(host: &DockerHost) -> Option<Arc<dyn ManagerEngine>> {
    let engine = match LocalDockerEngine::connect_for_manager(LocalEngineOptions {
        host: host.clone(),
        operation_timeout: Duration::from_secs(10),
    }) {
        Ok(engine) => engine,
        Err(error) => {
            tracing::warn!(%error, "manager engine is not configured; manager-scoped routes answer not_a_manager");
            return None;
        }
    };
    let cancellation = CancellationToken::new();
    match tokio::time::timeout(DESCRIBE_DEADLINE, engine.describe(&cancellation)).await {
        Ok(Ok(description))
            if description.capabilities.swarm_manager_operations == CapabilityStatus::Supported =>
        {
            Some(Arc::new(DockerManagerEngine(engine)))
        }
        Ok(Ok(_)) => {
            tracing::warn!(docker.host = %host, "settings.docker.host is not a Swarm manager; manager-scoped routes answer not_a_manager");
            None
        }
        Ok(Err(error)) => {
            tracing::warn!(%error, docker.host = %host, "manager engine is unreachable; manager-scoped routes answer not_a_manager");
            None
        }
        Err(_) => {
            tracing::warn!(docker.host = %host, "manager engine did not answer; manager-scoped routes answer not_a_manager");
            None
        }
    }
}
