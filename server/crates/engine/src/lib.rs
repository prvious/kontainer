#![forbid(unsafe_code)]

use std::{error::Error, fmt};

use async_trait::async_trait;
use bollard::Docker;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError {
    message: String,
}

impl EngineError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for EngineError {}

#[async_trait]
pub trait ContainerEngine: Send + Sync {
    async fn is_available(&self) -> Result<bool, EngineError>;
}

#[derive(Clone)]
pub struct LocalDockerEngine {
    client: Docker,
}

impl LocalDockerEngine {
    pub fn connect() -> Result<Self, EngineError> {
        Docker::connect_with_local_defaults()
            .map(|client| Self { client })
            .map_err(|error| EngineError::new(format!("cannot connect to local Docker: {error}")))
    }
}

#[async_trait]
impl ContainerEngine for LocalDockerEngine {
    async fn is_available(&self) -> Result<bool, EngineError> {
        self.client
            .ping()
            .await
            .map(|response| response == "OK")
            .map_err(|error| EngineError::new(format!("Docker ping failed: {error}")))
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
