#![forbid(unsafe_code)]

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

mod capability;
mod container;
mod error;
mod host;
mod local;
mod node;

pub use capability::{
    CapabilityStatus, EngineAccessScope, EngineApiVersion, EngineApiVersionParseError,
    EngineCapabilities, EngineDescription, UnsupportedCapabilityReason,
};
pub use container::{
    ContainerPage, ContainerPageRequest, ContainerState, ContainerSummary,
    DEFAULT_CONTAINER_PAGE_SIZE, MAX_CONTAINER_PAGE_SIZE, MAX_CONTAINER_PAGE_TOKEN_LEN,
    MAX_CONTAINER_STATUS_LEN, canonical_display_name, format_page_token, is_valid_container_id,
    parse_page_token, truncate_status_text,
};
pub use error::{EngineError, EngineErrorKind, EngineOperation};
pub use host::{DEFAULT_DOCKER_SOCKET, DockerHost, DockerHostParseError};
pub use local::{LocalDockerEngine, LocalEngineOptions};
pub use node::{ContainerStats, CpuCounters, DiskUsage, NodeInfo, cpu_percent, memory_used};

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
