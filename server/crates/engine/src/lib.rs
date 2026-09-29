#![forbid(unsafe_code)]

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

mod capability;
mod container;
mod error;
mod host;
mod image;
mod local;
mod network;
mod node;
mod service;
mod service_log;
mod volume;

pub use capability::{
    CapabilityStatus, EngineAccessScope, EngineApiVersion, EngineApiVersionParseError,
    EngineCapabilities, EngineDescription, UnsupportedCapabilityReason,
};
pub use container::{
    ContainerAction, ContainerActionOptions, ContainerPage, ContainerPageRequest, ContainerState,
    ContainerSummary, ContainerTimes, DEFAULT_CONTAINER_PAGE_SIZE, DEFAULT_STOP_TIMEOUT_SECONDS,
    MAX_CONTAINER_PAGE_SIZE, MAX_CONTAINER_PAGE_TOKEN_LEN, MAX_CONTAINER_STATUS_LEN,
    MAX_STOP_TIMEOUT_SECONDS, SERVICE_NAME_LABEL, STACK_NAMESPACE_LABEL, TASK_ID_LABEL,
    bounded_label, canonical_display_name, format_page_token, is_valid_container_id,
    parse_docker_time, parse_page_token, truncate_status_text,
};
pub use error::{EngineError, EngineErrorKind, EngineOperation};
pub use host::{DEFAULT_DOCKER_SOCKET, DockerHost, DockerHostParseError};
pub use image::{
    ImageSummary, PullEvent, PullProgress, PullStream, bounded_message, is_plausible_reference,
    is_valid_image_id,
};
pub use local::{LocalDockerEngine, LocalEngineOptions};
pub use network::{
    BUILTIN_NETWORKS, MAX_NETWORK_ENDPOINTS, MAX_NETWORK_NAME_LEN, NetworkCreate, NetworkDetail,
    NetworkEndpoint, NetworkSummary, is_builtin_network, is_valid_cidr, is_valid_network_id,
    is_valid_network_name,
};
pub use node::{ContainerStats, CpuCounters, DiskUsage, NodeInfo, cpu_percent, memory_used};
pub use service::{
    ServiceMode, ServiceSummary, SwarmAvailability, SwarmNode, SwarmNodeState, SwarmRole,
    SwarmStatus, TaskSummary, image_without_digest, is_valid_swarm_id,
};
pub use service_log::{
    LogSource, ServiceLogLine, ServiceLogRequest, ServiceLogStream, parse_service_log_line,
};
pub use volume::{
    MAX_VOLUME_LABELS, MAX_VOLUME_NAME_LEN, MAX_VOLUME_USERS, VolumeCreate, VolumeSummary,
    are_valid_volume_labels, is_valid_volume_name,
};

#[async_trait]
pub trait ContainerEngine: Send + Sync {
    async fn is_available(&self) -> Result<bool, EngineError>;

    async fn describe(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<EngineDescription, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::Describe,
        ))
    }

    async fn list_containers(
        &self,
        _request: ContainerPageRequest,
        _cancellation: &CancellationToken,
    ) -> Result<ContainerPage, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::ListContainers,
        ))
    }

    /// `docker info` for check-in.
    async fn node_info(&self, _cancellation: &CancellationToken) -> Result<NodeInfo, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::ReadSystemInfo,
        ))
    }

    /// Ids of every running container.
    async fn running_container_ids(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<String>, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::ListContainers,
        ))
    }

    /// One stats reading (two samples a second apart, taken by the daemon).
    async fn container_stats(
        &self,
        _container_id: &str,
        _cancellation: &CancellationToken,
    ) -> Result<ContainerStats, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::ReadContainerStats,
        ))
    }

    /// When the container last started and stopped (inspect).
    async fn container_times(
        &self,
        _container_id: &str,
        _cancellation: &CancellationToken,
    ) -> Result<ContainerTimes, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::InspectContainer,
        ))
    }

    /// Start, stop, restart, pause, unpause, or remove one container.
    async fn container_action(
        &self,
        _container_id: &str,
        _action: ContainerAction,
        _options: ContainerActionOptions,
        _cancellation: &CancellationToken,
    ) -> Result<(), EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::ContainerAction,
        ))
    }

    /// Every image on the node with how many of its containers use it.
    async fn list_images(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<ImageSummary>, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::ListImages,
        ))
    }

    /// Remove one image; `force` also removes it while containers use it.
    async fn remove_image(
        &self,
        _image_id: &str,
        _force: bool,
        _cancellation: &CancellationToken,
    ) -> Result<(), EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::RemoveImage,
        ))
    }

    /// Pull anonymously; the stream ends after the last progress message or one `Failed`.
    async fn pull_image(
        &self,
        _reference: &str,
        _cancellation: &CancellationToken,
    ) -> Result<PullStream, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::PullImage,
        ))
    }

    /// Every volume on the node with the containers that mount it. Sizes are left `None`: they
    /// come from [`ContainerEngine::disk_usage`], which callers sample sparingly.
    async fn list_volumes(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<VolumeSummary>, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::ListVolumes,
        ))
    }

    /// Create one volume. An existing name is [`EngineErrorKind::Conflict`]: Docker would
    /// silently hand back the existing volume instead.
    async fn create_volume(
        &self,
        _volume: &VolumeCreate,
        _cancellation: &CancellationToken,
    ) -> Result<VolumeSummary, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::CreateVolume,
        ))
    }

    /// Remove one volume. A volume a container mounts is [`EngineErrorKind::Conflict`], with or
    /// without force.
    async fn remove_volume(
        &self,
        _name: &str,
        _cancellation: &CancellationToken,
    ) -> Result<(), EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::RemoveVolume,
        ))
    }

    /// Every network this daemon knows, with its running endpoints here. On a manager that
    /// includes every swarm-scope network.
    async fn list_networks(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<NetworkSummary>, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::ListNetworks,
        ))
    }

    /// One network with the container endpoints this daemon has on it.
    async fn inspect_network(
        &self,
        _id: &str,
        _cancellation: &CancellationToken,
    ) -> Result<NetworkDetail, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::InspectNetwork,
        ))
    }

    /// Create one network and return its id.
    async fn create_network(
        &self,
        _network: &NetworkCreate,
        _cancellation: &CancellationToken,
    ) -> Result<String, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::CreateNetwork,
        ))
    }

    /// Remove one network. Built-ins are refused before Docker is asked.
    async fn remove_network(
        &self,
        _id: &str,
        _cancellation: &CancellationToken,
    ) -> Result<(), EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::RemoveNetwork,
        ))
    }

    /// Every Swarm service (manager only).
    async fn list_services(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<ServiceSummary>, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::ListServices,
        ))
    }

    /// The tasks of these services in one call; `current_only` keeps the ones meant to run.
    async fn list_tasks(
        &self,
        _service_ids: &[String],
        _current_only: bool,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<TaskSummary>, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::ListTasks,
        ))
    }

    /// Set a replicated service's replicas. `version` must match the service's current one.
    async fn scale_service(
        &self,
        _service_id: &str,
        _replicas: u64,
        _version: u64,
        _cancellation: &CancellationToken,
    ) -> Result<(), EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::UpdateService,
        ))
    }

    /// Restart every task by bumping `ForceUpdate`. `version` must match.
    async fn force_update_service(
        &self,
        _service_id: &str,
        _version: u64,
        _cancellation: &CancellationToken,
    ) -> Result<(), EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::UpdateService,
        ))
    }

    /// Roll back to the previous spec. `version` must match.
    async fn rollback_service(
        &self,
        _service_id: &str,
        _version: u64,
        _cancellation: &CancellationToken,
    ) -> Result<(), EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::UpdateService,
        ))
    }

    /// Remove one service; its tasks stop and go away after.
    async fn remove_service(
        &self,
        _service_id: &str,
        _cancellation: &CancellationToken,
    ) -> Result<(), EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::UpdateService,
        ))
    }

    /// Whether this daemon is a manager that can reach the Swarm's majority.
    async fn swarm_status(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<SwarmStatus, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::ReadSwarm,
        ))
    }

    /// A service's logs from every task, through the manager.
    async fn service_logs(
        &self,
        _service_id: &str,
        _request: ServiceLogRequest,
        _cancellation: &CancellationToken,
    ) -> Result<ServiceLogStream, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::ReadServiceLogs,
        ))
    }

    /// Every Swarm member, as the manager sees it.
    async fn list_swarm_nodes(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<SwarmNode>, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::ReadSwarm,
        ))
    }

    /// `docker system df`; expensive, so callers sample it sparingly.
    async fn disk_usage(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<DiskUsage, EngineError> {
        Err(EngineError::new(
            EngineErrorKind::UnsupportedCapability,
            EngineOperation::ReadDiskUsage,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{ContainerEngine, EngineError};
    use async_trait::async_trait;

    struct UnavailableEngine;

    #[async_trait]
    impl ContainerEngine for UnavailableEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(false)
        }
    }

    #[tokio::test]
    async fn engine_availability_is_exposed_through_the_owned_port() {
        assert!(!UnavailableEngine.is_available().await.unwrap());
    }
}
