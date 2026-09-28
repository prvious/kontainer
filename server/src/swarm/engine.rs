//! `ManagerEngine`: the controller-owned port for swarm-wide objects (networks now, services in
//! spec 11). The Docker implementation wraps the engine crate's manager-scoped engine built from
//! `settings.docker.host`; a host that is not a Swarm manager leaves the port absent and
//! manager-scoped routes answer 503 `not_a_manager`.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use kontainer_engine::{
    CapabilityStatus, ContainerEngine, DockerHost, EngineErrorKind, LocalDockerEngine,
    LocalEngineOptions, NetworkCreate, NetworkDetail, NetworkSummary, ServiceSummary, SwarmStatus,
    TaskSummary,
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
    /// The Swarm has no leader: the manager can't read or change cluster state.
    QuorumUnavailable,
    /// The object changed since the version the request was based on.
    VersionConflict,
}

impl std::fmt::Display for ManagerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "manager engine is unavailable",
            Self::NotFound => "object not found on the manager",
            Self::InvalidInput => "request refused",
            Self::Rejected => "the manager refused the change",
            Self::Cancelled => "manager call was cancelled",
            Self::QuorumUnavailable => "the swarm has no quorum",
            Self::VersionConflict => "the object changed since it was read",
        })
    }
}

impl From<kontainer_engine::EngineError> for ManagerError {
    fn from(error: kontainer_engine::EngineError) -> Self {
        match error.kind() {
            EngineErrorKind::NotFound => Self::NotFound,
            EngineErrorKind::InvalidInput => Self::InvalidInput,
            EngineErrorKind::Cancelled => Self::Cancelled,
            EngineErrorKind::QuorumUnavailable => Self::QuorumUnavailable,
            EngineErrorKind::VersionConflict => Self::VersionConflict,
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

    async fn list_services(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<ServiceSummary>, ManagerError> {
        Err(ManagerError::Unavailable)
    }

    /// The tasks of these services in one call; `current_only` keeps the ones meant to run.
    async fn list_tasks(
        &self,
        _service_ids: &[String],
        _current_only: bool,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<TaskSummary>, ManagerError> {
        Err(ManagerError::Unavailable)
    }

    async fn scale_service(
        &self,
        _service_id: &str,
        _replicas: u64,
        _version: u64,
        _cancellation: &CancellationToken,
    ) -> Result<(), ManagerError> {
        Err(ManagerError::Unavailable)
    }

    async fn force_update_service(
        &self,
        _service_id: &str,
        _version: u64,
        _cancellation: &CancellationToken,
    ) -> Result<(), ManagerError> {
        Err(ManagerError::Unavailable)
    }

    async fn rollback_service(
        &self,
        _service_id: &str,
        _version: u64,
        _cancellation: &CancellationToken,
    ) -> Result<(), ManagerError> {
        Err(ManagerError::Unavailable)
    }

    async fn swarm_status(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<SwarmStatus, ManagerError> {
        Err(ManagerError::Unavailable)
    }
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

    async fn list_services(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<ServiceSummary>, ManagerError> {
        Ok(self.0.list_services(cancellation).await?)
    }

    async fn list_tasks(
        &self,
        service_ids: &[String],
        current_only: bool,
        cancellation: &CancellationToken,
    ) -> Result<Vec<TaskSummary>, ManagerError> {
        Ok(self
            .0
            .list_tasks(service_ids, current_only, cancellation)
            .await?)
    }

    async fn scale_service(
        &self,
        service_id: &str,
        replicas: u64,
        version: u64,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagerError> {
        Ok(self
            .0
            .scale_service(service_id, replicas, version, cancellation)
            .await?)
    }

    async fn force_update_service(
        &self,
        service_id: &str,
        version: u64,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagerError> {
        Ok(self
            .0
            .force_update_service(service_id, version, cancellation)
            .await?)
    }

    async fn rollback_service(
        &self,
        service_id: &str,
        version: u64,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagerError> {
        Ok(self
            .0
            .rollback_service(service_id, version, cancellation)
            .await?)
    }

    async fn swarm_status(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<SwarmStatus, ManagerError> {
        Ok(self.0.swarm_status(cancellation).await?)
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
