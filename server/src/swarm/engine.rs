//! `ManagerEngine`: the controller-owned port for swarm-wide objects: networks, services, tasks,
//! and Swarm members. The Docker implementation wraps the engine crate's manager-scoped engine
//! built from `settings.docker.host`. Whether that host is a Swarm manager is checked per call
//! (ADR 0031): while it isn't, every call fails `NotAManager` and routes answer 503
//! `not_a_manager`.

use std::{
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use kontainer_engine::{
    CapabilityStatus, ContainerEngine, DockerHost, EngineErrorKind, LocalDockerEngine,
    LocalEngineOptions, NetworkCreate, NetworkDetail, NetworkSummary, ServiceLogRequest,
    ServiceLogStream, ServiceSummary, SwarmNode, SwarmStatus, TaskSummary,
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
    /// The configured host is not a Swarm manager (a worker, or not in a Swarm).
    NotAManager,
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
            Self::NotAManager => "the configured Docker host is not a Swarm manager",
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
            EngineErrorKind::NotAManager => Self::NotAManager,
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

    async fn remove_service(
        &self,
        _service_id: &str,
        _cancellation: &CancellationToken,
    ) -> Result<(), ManagerError> {
        Err(ManagerError::Unavailable)
    }

    async fn list_swarm_nodes(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<SwarmNode>, ManagerError> {
        Err(ManagerError::Unavailable)
    }

    /// A service's logs from every task; errors mid-stream arrive as items.
    async fn service_logs(
        &self,
        _service_id: &str,
        _request: ServiceLogRequest,
        _cancellation: &CancellationToken,
    ) -> Result<ServiceLogStream, ManagerError> {
        Err(ManagerError::Unavailable)
    }
}

/// The manager engine routes and executors use. It is present unless `settings.docker.host` can't
/// be turned into an engine; whether that host is a manager is decided per call.
#[derive(Clone)]
pub struct SharedManagerEngine(pub Arc<dyn ManagerEngine>);

/// How long one `describe()` answer about the host's Swarm role is trusted. A demotion inside it
/// still fails `NotAManager`, from Docker's own refusal.
const ROLE_TTL: Duration = Duration::from_secs(30);
const DESCRIBE_DEADLINE: Duration = Duration::from_secs(5);

/// The Docker manager, through the engine crate. Every call first checks that the host is a Swarm
/// manager, because a worker answers `network ls` with only its local view.
pub struct DockerManagerEngine<E = LocalDockerEngine> {
    engine: E,
    /// When `describe()` last answered, and whether it said "manager".
    role: Mutex<Option<(Instant, bool)>>,
}

impl<E: ContainerEngine> DockerManagerEngine<E> {
    pub const fn new(engine: E) -> Self {
        Self {
            engine,
            role: Mutex::new(None),
        }
    }

    /// The engine, once the host is known to be a manager. A `describe()` that fails (daemon down
    /// or slow) is not remembered: this call fails as unavailable and the next one asks again.
    async fn manager(&self, cancellation: &CancellationToken) -> Result<&E, ManagerError> {
        let known = *self.role.lock().unwrap_or_else(PoisonError::into_inner);
        let is_manager = match known {
            Some((checked_at, is_manager)) if checked_at.elapsed() < ROLE_TTL => is_manager,
            _ => {
                let description = match tokio::time::timeout(
                    DESCRIBE_DEADLINE,
                    self.engine.describe(cancellation),
                )
                .await
                {
                    Ok(Ok(description)) => description,
                    Ok(Err(error)) if error.kind() == EngineErrorKind::Cancelled => {
                        return Err(ManagerError::Cancelled);
                    }
                    Ok(Err(_)) | Err(_) => return Err(ManagerError::Unavailable),
                };
                let is_manager = description.capabilities.swarm_manager_operations
                    == CapabilityStatus::Supported;
                if !is_manager && known.is_none_or(|(_, was_manager)| was_manager) {
                    tracing::warn!(
                        "settings.docker.host is not a Swarm manager; manager-scoped routes answer not_a_manager"
                    );
                }
                *self.role.lock().unwrap_or_else(PoisonError::into_inner) =
                    Some((Instant::now(), is_manager));
                is_manager
            }
        };
        if is_manager {
            Ok(&self.engine)
        } else {
            Err(ManagerError::NotAManager)
        }
    }
}

#[async_trait]
impl<E: ContainerEngine> ManagerEngine for DockerManagerEngine<E> {
    async fn list_networks(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<NetworkSummary>, ManagerError> {
        let engine = self.manager(cancellation).await?;
        Ok(engine.list_networks(cancellation).await?)
    }

    async fn inspect_network(
        &self,
        id: &str,
        cancellation: &CancellationToken,
    ) -> Result<NetworkDetail, ManagerError> {
        let engine = self.manager(cancellation).await?;
        Ok(engine.inspect_network(id, cancellation).await?)
    }

    async fn create_network(
        &self,
        network: &NetworkCreate,
        cancellation: &CancellationToken,
    ) -> Result<String, ManagerError> {
        let engine = self.manager(cancellation).await?;
        Ok(engine.create_network(network, cancellation).await?)
    }

    async fn remove_network(
        &self,
        id: &str,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagerError> {
        let engine = self.manager(cancellation).await?;
        Ok(engine.remove_network(id, cancellation).await?)
    }

    async fn list_services(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<ServiceSummary>, ManagerError> {
        let engine = self.manager(cancellation).await?;
        Ok(engine.list_services(cancellation).await?)
    }

    async fn list_tasks(
        &self,
        service_ids: &[String],
        current_only: bool,
        cancellation: &CancellationToken,
    ) -> Result<Vec<TaskSummary>, ManagerError> {
        let engine = self.manager(cancellation).await?;
        Ok(engine
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
        let engine = self.manager(cancellation).await?;
        Ok(engine
            .scale_service(service_id, replicas, version, cancellation)
            .await?)
    }

    async fn force_update_service(
        &self,
        service_id: &str,
        version: u64,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagerError> {
        let engine = self.manager(cancellation).await?;
        Ok(engine
            .force_update_service(service_id, version, cancellation)
            .await?)
    }

    async fn rollback_service(
        &self,
        service_id: &str,
        version: u64,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagerError> {
        let engine = self.manager(cancellation).await?;
        Ok(engine
            .rollback_service(service_id, version, cancellation)
            .await?)
    }

    async fn swarm_status(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<SwarmStatus, ManagerError> {
        let engine = self.manager(cancellation).await?;
        Ok(engine.swarm_status(cancellation).await?)
    }

    async fn remove_service(
        &self,
        service_id: &str,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagerError> {
        let engine = self.manager(cancellation).await?;
        Ok(engine.remove_service(service_id, cancellation).await?)
    }

    async fn list_swarm_nodes(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<SwarmNode>, ManagerError> {
        let engine = self.manager(cancellation).await?;
        Ok(engine.list_swarm_nodes(cancellation).await?)
    }

    async fn service_logs(
        &self,
        service_id: &str,
        request: ServiceLogRequest,
        cancellation: &CancellationToken,
    ) -> Result<ServiceLogStream, ManagerError> {
        let engine = self.manager(cancellation).await?;
        Ok(engine
            .service_logs(service_id, request, cancellation)
            .await?)
    }
}

/// The manager engine for `host`. Building it does no I/O, so it exists even while the daemon is
/// down or not yet a manager; `None` only for a host the engine can't use, logged once.
pub fn connect_manager(host: &DockerHost) -> Option<Arc<dyn ManagerEngine>> {
    match LocalDockerEngine::connect_for_manager(LocalEngineOptions {
        host: host.clone(),
        operation_timeout: Duration::from_secs(10),
    }) {
        Ok(engine) => Some(Arc::new(DockerManagerEngine::new(engine))),
        Err(error) => {
            tracing::warn!(%error, docker.host = %host, "manager engine is not configured; manager-scoped routes answer not_a_manager");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use async_trait::async_trait;
    use kontainer_engine::{
        CapabilityStatus, ContainerEngine, EngineAccessScope, EngineApiVersion, EngineCapabilities,
        EngineDescription, EngineError, EngineErrorKind, EngineOperation, NetworkSummary,
        UnsupportedCapabilityReason,
    };
    use tokio_util::sync::CancellationToken;

    use super::{DockerManagerEngine, ManagerEngine, ManagerError};

    /// Answers `describe()` from a queue: `Some(true)` a manager, `Some(false)` a worker, `None`
    /// a daemon that doesn't answer.
    struct FakeDocker {
        describes: Mutex<VecDeque<Option<bool>>>,
        describe_calls: AtomicUsize,
    }

    impl FakeDocker {
        fn answering(answers: &[Option<bool>]) -> Self {
            Self {
                describes: Mutex::new(answers.iter().copied().collect()),
                describe_calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl ContainerEngine for FakeDocker {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }

        async fn describe(&self, _: &CancellationToken) -> Result<EngineDescription, EngineError> {
            self.describe_calls.fetch_add(1, Ordering::SeqCst);
            let manager = self
                .describes
                .lock()
                .unwrap()
                .pop_front()
                .flatten()
                .ok_or_else(|| {
                    EngineError::new(
                        EngineErrorKind::ConnectionUnavailable,
                        EngineOperation::Describe,
                    )
                })?;
            let version = EngineApiVersion::new(1, 47);
            Ok(EngineDescription {
                access_scope: EngineAccessScope::Manager,
                daemon_version: "28.0.0".to_owned(),
                daemon_api_version: version,
                minimum_api_version: version,
                negotiated_api_version: version,
                operating_system: "linux".to_owned(),
                architecture: "x86_64".to_owned(),
                capabilities: EngineCapabilities {
                    local_container_operations: CapabilityStatus::Supported,
                    swarm_node_operations: CapabilityStatus::Supported,
                    swarm_manager_operations: if manager {
                        CapabilityStatus::Supported
                    } else {
                        CapabilityStatus::Unsupported(
                            UnsupportedCapabilityReason::ManagerControlUnavailable,
                        )
                    },
                },
            })
        }

        async fn list_networks(
            &self,
            _: &CancellationToken,
        ) -> Result<Vec<NetworkSummary>, EngineError> {
            Ok(vec![NetworkSummary::default()])
        }
    }

    #[tokio::test]
    async fn a_daemon_that_was_down_is_used_once_it_answers() {
        let manager = DockerManagerEngine::new(FakeDocker::answering(&[None, Some(true)]));
        let cancellation = CancellationToken::new();

        assert_eq!(
            manager.list_networks(&cancellation).await.err(),
            Some(ManagerError::Unavailable)
        );
        for _ in 0..2 {
            assert_eq!(
                manager
                    .list_networks(&cancellation)
                    .await
                    .map(|networks| networks.len()),
                Ok(1)
            );
        }
        assert_eq!(
            manager.engine.describe_calls.load(Ordering::SeqCst),
            2,
            "the failure is not remembered; the success is"
        );
    }

    #[tokio::test]
    async fn a_host_that_is_not_a_manager_refuses_every_call() {
        let manager = DockerManagerEngine::new(FakeDocker::answering(&[Some(false)]));
        let cancellation = CancellationToken::new();

        assert_eq!(
            manager.list_networks(&cancellation).await.err(),
            Some(ManagerError::NotAManager),
            "a worker's local network list is not the swarm's"
        );
        assert_eq!(
            manager.list_services(&cancellation).await.err(),
            Some(ManagerError::NotAManager)
        );
        assert_eq!(manager.engine.describe_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn docker_refusing_a_non_manager_maps_to_not_a_manager() {
        let error = EngineError::new(EngineErrorKind::NotAManager, EngineOperation::ListServices);
        assert_eq!(ManagerError::from(error), ManagerError::NotAManager);
    }
}
