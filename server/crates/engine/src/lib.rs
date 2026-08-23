#![forbid(unsafe_code)]

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

mod capability;
mod error;
mod local;

pub use capability::{
    CapabilityStatus, EngineAccessScope, EngineApiVersion, EngineCapabilities, EngineCapability,
    EngineDescription, UnsupportedCapabilityReason,
};
pub use error::{EngineError, EngineErrorKind, EngineOperation};
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
