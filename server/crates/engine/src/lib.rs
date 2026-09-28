#![forbid(unsafe_code)]

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

mod capability;
mod container;
mod error;
mod host;
mod local;

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
