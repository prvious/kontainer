use std::{future::Future, path::PathBuf, time::Duration};

use async_trait::async_trait;
use bollard::{
    API_DEFAULT_VERSION, Docker,
    errors::Error as BollardError,
    models::{LocalNodeState, SwarmInfo},
};
use tokio::time;
use tokio_util::sync::CancellationToken;

use crate::{
    CapabilityStatus, ContainerEngine, EngineAccessScope, EngineApiVersion, EngineCapabilities,
    EngineDescription, EngineError, EngineErrorKind, EngineOperation, UnsupportedCapabilityReason,
};

const DEFAULT_DOCKER_SOCKET: &str = "/var/run/docker.sock";
const DEFAULT_OPERATION_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalEngineOptions {
    pub socket_path: PathBuf,
    pub operation_timeout: Duration,
}

impl Default for LocalEngineOptions {
    fn default() -> Self {
        Self {
            socket_path: DEFAULT_DOCKER_SOCKET.into(),
            operation_timeout: DEFAULT_OPERATION_TIMEOUT,
        }
    }
}

#[derive(Clone)]
pub struct LocalDockerEngine {
    access_scope: EngineAccessScope,
    client: Docker,
    operation_timeout: Duration,
}

impl LocalDockerEngine {
    pub fn connect_for_manager(options: LocalEngineOptions) -> Result<Self, EngineError> {
        Self::connect(EngineAccessScope::Manager, options)
    }

    pub fn connect_for_node(options: LocalEngineOptions) -> Result<Self, EngineError> {
        Self::connect(EngineAccessScope::NodeLocal, options)
    }

    fn connect(
        access_scope: EngineAccessScope,
        options: LocalEngineOptions,
    ) -> Result<Self, EngineError> {
        if options.operation_timeout.is_zero() {
            return Err(EngineError::new(
                EngineErrorKind::InvalidConfiguration,
                EngineOperation::Connect,
            ));
        }

        let socket_path = options.socket_path.to_str().ok_or_else(|| {
            EngineError::new(
                EngineErrorKind::InvalidConfiguration,
                EngineOperation::Connect,
            )
        })?;
        let bollard_timeout = options.operation_timeout.as_secs().max(1);
        let client = Docker::connect_with_socket(socket_path, bollard_timeout, API_DEFAULT_VERSION)
            .map_err(|error| EngineError::from_bollard(EngineOperation::Connect, &error))?;

        Ok(Self {
            access_scope,
            client,
            operation_timeout: options.operation_timeout,
        })
    }

    async fn negotiate(&self, cancellation: &CancellationToken) -> Result<Docker, EngineError> {
        self.execute(
            EngineOperation::NegotiateApiVersion,
            cancellation,
            self.client.clone().negotiate_version(),
        )
        .await
    }

    async fn execute<T, F>(
        &self,
        operation: EngineOperation,
        cancellation: &CancellationToken,
        future: F,
    ) -> Result<T, EngineError>
    where
        F: Future<Output = Result<T, BollardError>>,
    {
        tokio::select! {
            biased;
            () = cancellation.cancelled() => Err(EngineError::new(
                EngineErrorKind::Cancelled,
                operation,
            )),
            result = time::timeout(self.operation_timeout, future) => match result {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(error)) => Err(EngineError::from_bollard(operation, &error)),
                Err(_) => Err(EngineError::new(EngineErrorKind::DeadlineExceeded, operation)),
            },
        }
    }
}

#[async_trait]
impl ContainerEngine for LocalDockerEngine {
    async fn is_available(&self) -> Result<bool, EngineError> {
        let cancellation = CancellationToken::new();
        let client = self.negotiate(&cancellation).await?;
        let response = self
            .execute(EngineOperation::Ping, &cancellation, client.ping())
            .await?;

        if response == "OK" {
            Ok(true)
        } else {
            Err(EngineError::new(
                EngineErrorKind::InvalidResponse,
                EngineOperation::Ping,
            ))
        }
    }

    async fn describe(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<EngineDescription, EngineError> {
        let client = self.negotiate(cancellation).await?;
        let ping = self
            .execute(EngineOperation::Ping, cancellation, client.ping())
            .await?;
        if ping != "OK" {
            return Err(EngineError::new(
                EngineErrorKind::InvalidResponse,
                EngineOperation::Ping,
            ));
        }

        let version = self
            .execute(EngineOperation::ReadVersion, cancellation, client.version())
            .await?;
        let client_version = client.client_version();
        let negotiated_api_version = EngineApiVersion::new(
            u16::try_from(client_version.major_version).map_err(|_| {
                EngineError::new(EngineErrorKind::InvalidResponse, EngineOperation::Describe)
            })?,
            u16::try_from(client_version.minor_version).map_err(|_| {
                EngineError::new(EngineErrorKind::InvalidResponse, EngineOperation::Describe)
            })?,
        );
        let daemon_api_version = parse_required_api_version(
            version.api_version.as_deref(),
            EngineOperation::ReadVersion,
        )?;
        let minimum_api_version = parse_required_api_version(
            version.min_api_version.as_deref(),
            EngineOperation::ReadVersion,
        )?;

        if negotiated_api_version < minimum_api_version
            || negotiated_api_version > daemon_api_version
        {
            return Err(EngineError::new(
                EngineErrorKind::UnsupportedApiVersion,
                EngineOperation::NegotiateApiVersion,
            ));
        }

        let system_info = self
            .execute(EngineOperation::ReadSystemInfo, cancellation, client.info())
            .await?;
        let daemon_version = required_text(version.version, EngineOperation::ReadVersion)?;
        let operating_system = required_text(
            system_info.operating_system.or(version.os),
            EngineOperation::ReadSystemInfo,
        )?;
        let architecture = required_text(
            system_info.architecture.or(version.arch),
            EngineOperation::ReadSystemInfo,
        )?;
        let capabilities = capabilities_from_swarm(system_info.swarm);

        Ok(EngineDescription {
            access_scope: self.access_scope,
            daemon_version,
            daemon_api_version,
            minimum_api_version,
            negotiated_api_version,
            operating_system,
            architecture,
            capabilities,
        })
    }
}

fn parse_required_api_version(
    value: Option<&str>,
    operation: EngineOperation,
) -> Result<EngineApiVersion, EngineError> {
    value
        .ok_or_else(|| EngineError::new(EngineErrorKind::InvalidResponse, operation))?
        .parse()
        .map_err(|_| EngineError::new(EngineErrorKind::InvalidResponse, operation))
}

fn required_text(value: Option<String>, operation: EngineOperation) -> Result<String, EngineError> {
    value
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| EngineError::new(EngineErrorKind::InvalidResponse, operation))
}

fn capabilities_from_swarm(swarm: Option<SwarmInfo>) -> EngineCapabilities {
    let (node, manager) = match swarm.and_then(|swarm| {
        swarm
            .local_node_state
            .map(|state| (state, swarm.control_available))
    }) {
        Some((LocalNodeState::ACTIVE, Some(true))) => {
            (CapabilityStatus::Supported, CapabilityStatus::Supported)
        }
        Some((LocalNodeState::ACTIVE, _)) => (
            CapabilityStatus::Supported,
            CapabilityStatus::Unsupported(UnsupportedCapabilityReason::ManagerControlUnavailable),
        ),
        Some((state, _)) => {
            let reason = match state {
                LocalNodeState::INACTIVE | LocalNodeState::EMPTY => {
                    UnsupportedCapabilityReason::SwarmInactive
                }
                LocalNodeState::PENDING => UnsupportedCapabilityReason::SwarmPending,
                LocalNodeState::ERROR => UnsupportedCapabilityReason::SwarmError,
                LocalNodeState::LOCKED => UnsupportedCapabilityReason::SwarmLocked,
                LocalNodeState::ACTIVE => unreachable!("active Swarm states are handled above"),
            };
            (
                CapabilityStatus::Unsupported(reason),
                CapabilityStatus::Unsupported(reason),
            )
        }
        None => (
            CapabilityStatus::Unsupported(UnsupportedCapabilityReason::SwarmStateUnavailable),
            CapabilityStatus::Unsupported(UnsupportedCapabilityReason::SwarmStateUnavailable),
        ),
    };

    EngineCapabilities {
        local_container_operations: CapabilityStatus::Supported,
        swarm_node_operations: node,
        swarm_manager_operations: manager,
    }
}

#[cfg(test)]
mod tests {
    use std::{future, path::PathBuf, time::Duration};

    use bollard::models::{LocalNodeState, SwarmInfo};
    use tokio_util::sync::CancellationToken;

    use super::{LocalDockerEngine, LocalEngineOptions, capabilities_from_swarm};
    use crate::{
        CapabilityStatus, ContainerEngine, EngineAccessScope, EngineErrorKind, EngineOperation,
        UnsupportedCapabilityReason,
    };

    #[test]
    fn manager_and_node_constructors_preserve_their_authority() {
        let missing_socket = PathBuf::from("/definitely/missing/kontainer-docker.sock");
        for connect in [
            LocalDockerEngine::connect_for_manager
                as fn(LocalEngineOptions) -> Result<LocalDockerEngine, crate::EngineError>,
            LocalDockerEngine::connect_for_node
                as fn(LocalEngineOptions) -> Result<LocalDockerEngine, crate::EngineError>,
        ] {
            let error = connect(LocalEngineOptions {
                socket_path: missing_socket.clone(),
                operation_timeout: Duration::from_secs(1),
            })
            .err()
            .unwrap();
            assert_eq!(error.kind(), EngineErrorKind::ConnectionUnavailable);
            assert_eq!(error.operation(), EngineOperation::Connect);
        }

        assert_ne!(EngineAccessScope::Manager, EngineAccessScope::NodeLocal);
    }

    #[test]
    fn zero_timeout_is_rejected_before_adapter_construction() {
        let error = LocalDockerEngine::connect_for_node(LocalEngineOptions {
            socket_path: "/var/run/docker.sock".into(),
            operation_timeout: Duration::ZERO,
        })
        .err()
        .unwrap();

        assert_eq!(error.kind(), EngineErrorKind::InvalidConfiguration);
    }

    #[test]
    fn inactive_and_worker_swarms_report_manager_support_explicitly() {
        let inactive = capabilities_from_swarm(Some(SwarmInfo {
            local_node_state: Some(LocalNodeState::INACTIVE),
            control_available: Some(false),
            ..Default::default()
        }));
        assert_eq!(
            inactive.swarm_node_operations,
            CapabilityStatus::Unsupported(UnsupportedCapabilityReason::SwarmInactive)
        );

        let worker = capabilities_from_swarm(Some(SwarmInfo {
            local_node_state: Some(LocalNodeState::ACTIVE),
            control_available: Some(false),
            ..Default::default()
        }));
        assert_eq!(worker.swarm_node_operations, CapabilityStatus::Supported);
        assert_eq!(
            worker.swarm_manager_operations,
            CapabilityStatus::Unsupported(UnsupportedCapabilityReason::ManagerControlUnavailable)
        );
    }

    #[test]
    fn missing_manager_control_does_not_discard_known_node_state() {
        for (state, expected_node) in [
            (LocalNodeState::ACTIVE, CapabilityStatus::Supported),
            (
                LocalNodeState::INACTIVE,
                CapabilityStatus::Unsupported(UnsupportedCapabilityReason::SwarmInactive),
            ),
            (
                LocalNodeState::PENDING,
                CapabilityStatus::Unsupported(UnsupportedCapabilityReason::SwarmPending),
            ),
        ] {
            let capabilities = capabilities_from_swarm(Some(SwarmInfo {
                local_node_state: Some(state),
                control_available: None,
                ..Default::default()
            }));

            assert_eq!(capabilities.swarm_node_operations, expected_node);
            assert_ne!(
                capabilities.swarm_manager_operations,
                CapabilityStatus::Supported
            );
        }
    }

    #[tokio::test]
    async fn unsupported_api_is_reported_before_requesting_system_info() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::UnixListener,
        };

        let socket_path = std::env::temp_dir().join(format!(
            "kontainer-engine-api-range-{}.sock",
            std::process::id(),
        ));
        let listener = UnixListener::bind(&socket_path).unwrap();
        let engine = LocalDockerEngine::connect_for_node(LocalEngineOptions {
            socket_path: socket_path.clone(),
            operation_timeout: Duration::from_secs(1),
        })
        .unwrap();
        let fixture = tokio::spawn(async move {
            for expected_endpoint in ["/version", "/_ping", "/version"] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                loop {
                    let byte = socket.read_u8().await.unwrap();
                    request.push(byte);
                    if request.ends_with(b"\r\n\r\n") {
                        break;
                    }
                    assert!(request.len() < 4096);
                }
                let request = String::from_utf8(request).unwrap();
                assert!(request.lines().next().unwrap().contains(expected_endpoint));
                let body = if expected_endpoint == "/_ping" {
                    "OK"
                } else {
                    r#"{"ApiVersion":"1.99","MinAPIVersion":"1.99","Version":"future"}"#
                };
                socket
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len(),
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
            }
        });

        let result = engine.describe(&CancellationToken::new()).await;
        tokio::time::timeout(Duration::from_secs(2), fixture)
            .await
            .unwrap()
            .unwrap();
        std::fs::remove_file(socket_path).unwrap();

        let error = result.unwrap_err();
        assert_eq!(error.kind(), EngineErrorKind::UnsupportedApiVersion);
        assert_eq!(error.operation(), EngineOperation::NegotiateApiVersion);
    }

    #[tokio::test]
    async fn cancellation_wins_over_a_pending_engine_request() {
        let engine = test_engine(Duration::from_secs(1));
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let error = engine
            .execute(
                EngineOperation::Ping,
                &cancellation,
                future::pending::<Result<(), bollard::errors::Error>>(),
            )
            .await
            .unwrap_err();

        assert_eq!(error.kind(), EngineErrorKind::Cancelled);
    }

    #[tokio::test]
    async fn operation_deadline_bounds_a_pending_engine_request() {
        let engine = test_engine(Duration::from_millis(10));
        let cancellation = CancellationToken::new();

        let error = engine
            .execute(
                EngineOperation::ReadSystemInfo,
                &cancellation,
                future::pending::<Result<(), bollard::errors::Error>>(),
            )
            .await
            .unwrap_err();

        assert_eq!(error.kind(), EngineErrorKind::DeadlineExceeded);
    }

    fn test_engine(operation_timeout: Duration) -> LocalDockerEngine {
        let socket_path = std::env::temp_dir().join(format!(
            "kontainer-engine-test-{}-{}.sock",
            std::process::id(),
            operation_timeout.as_nanos()
        ));
        let listener = std::os::unix::net::UnixListener::bind(&socket_path).unwrap();
        let client = bollard::Docker::connect_with_unix(
            socket_path.to_str().unwrap(),
            1,
            bollard::API_DEFAULT_VERSION,
        )
        .unwrap();
        drop(listener);
        std::fs::remove_file(socket_path).unwrap();

        LocalDockerEngine {
            access_scope: EngineAccessScope::NodeLocal,
            client,
            operation_timeout,
        }
    }
}
