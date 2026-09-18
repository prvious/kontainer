use std::{collections::HashMap, future::Future, path::PathBuf, time::Duration};

use async_trait::async_trait;
use bollard::{
    API_DEFAULT_VERSION, Docker,
    errors::Error as BollardError,
    models::{ContainerSummaryStateEnum, LocalNodeState, SwarmInfo},
};
use tokio::time;
use tokio_util::sync::CancellationToken;

use crate::{
    CapabilityStatus, ContainerEngine, ContainerPage, ContainerPageRequest, ContainerState,
    ContainerSummary, EngineAccessScope, EngineApiVersion, EngineCapabilities, EngineDescription,
    EngineError, EngineErrorKind, EngineOperation, UnsupportedCapabilityReason,
    canonical_display_name, format_page_token, is_valid_container_id, truncate_status_text,
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

    async fn list_containers(
        &self,
        request: ContainerPageRequest,
        cancellation: &CancellationToken,
    ) -> Result<ContainerPage, EngineError> {
        use bollard::query_parameters::ListContainersOptionsBuilder;

        let client = self.negotiate(cancellation).await?;
        let mut options = ListContainersOptionsBuilder::new()
            .all(true)
            .size(false)
            .limit(request.daemon_limit());
        if let Some(cursor) = request.cursor() {
            let mut filters = HashMap::new();
            filters.insert("before".to_owned(), vec![cursor.to_owned()]);
            options = options.filters(&filters);
        }
        let summaries = self
            .execute(
                EngineOperation::ListContainers,
                cancellation,
                client.list_containers(Some(options.build())),
            )
            .await?;

        let mut containers = Vec::with_capacity(request.effective_page_size());
        for summary in &summaries {
            containers.push(map_container_summary(summary)?);
        }
        if containers.len() > request.effective_page_size() {
            containers.truncate(request.effective_page_size());
            let cursor = containers
                .last()
                .map(|last| format_page_token(&last.container_id))
                .ok_or_else(|| {
                    EngineError::new(
                        EngineErrorKind::InvalidResponse,
                        EngineOperation::ListContainers,
                    )
                })?;
            Ok(ContainerPage {
                containers,
                next_page_token: Some(cursor),
            })
        } else {
            Ok(ContainerPage {
                containers,
                next_page_token: None,
            })
        }
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

fn invalid_containers() -> EngineError {
    EngineError::new(
        EngineErrorKind::InvalidResponse,
        EngineOperation::ListContainers,
    )
}

/// Map one daemon summary to the safe domain shape. Only approved fields
/// cross the boundary; command, labels, ports, mounts, networking, and raw
/// inspect data are dropped here.
fn map_container_summary(
    summary: &bollard::models::ContainerSummary,
) -> Result<ContainerSummary, EngineError> {
    let container_id = summary
        .id
        .clone()
        .filter(|id| is_valid_container_id(id))
        .ok_or_else(invalid_containers)?;
    let display_name = canonical_display_name(summary.names.as_deref().unwrap_or_default())
        .ok_or_else(invalid_containers)?;
    let image_reference = summary
        .image
        .clone()
        .filter(|image| !image.trim().is_empty())
        .ok_or_else(invalid_containers)?;
    let created_at_seconds = summary
        .created
        .filter(|created| *created >= 0)
        .ok_or_else(invalid_containers)?;

    Ok(ContainerSummary {
        container_id,
        display_name,
        image_reference,
        state: map_container_state(summary.state),
        status: truncate_status_text(summary.status.as_deref().unwrap_or_default()),
        created_at_seconds,
    })
}

fn map_container_state(state: Option<ContainerSummaryStateEnum>) -> ContainerState {
    match state {
        Some(ContainerSummaryStateEnum::CREATED) => ContainerState::Created,
        Some(ContainerSummaryStateEnum::RUNNING) => ContainerState::Running,
        Some(ContainerSummaryStateEnum::PAUSED) => ContainerState::Paused,
        Some(ContainerSummaryStateEnum::RESTARTING) => ContainerState::Restarting,
        Some(ContainerSummaryStateEnum::REMOVING) => ContainerState::Removing,
        Some(ContainerSummaryStateEnum::EXITED) => ContainerState::Exited,
        Some(ContainerSummaryStateEnum::DEAD) => ContainerState::Dead,
        Some(ContainerSummaryStateEnum::STOPPING) => ContainerState::Stopping,
        Some(ContainerSummaryStateEnum::EMPTY) | None => ContainerState::Unknown,
    }
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

    use bollard::models::{ContainerSummaryStateEnum, LocalNodeState, SwarmInfo};
    use tokio_util::sync::CancellationToken;

    use super::{
        LocalDockerEngine, LocalEngineOptions, capabilities_from_swarm, map_container_state,
        map_container_summary,
    };
    use crate::{
        CapabilityStatus, ContainerEngine, ContainerPageRequest, ContainerState, EngineAccessScope,
        EngineErrorKind, EngineOperation, UnsupportedCapabilityReason,
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

    #[test]
    fn container_states_keep_distinct_variants_with_unknown_fallback() {
        for (wire, expected) in [
            (
                Some(ContainerSummaryStateEnum::CREATED),
                ContainerState::Created,
            ),
            (
                Some(ContainerSummaryStateEnum::RUNNING),
                ContainerState::Running,
            ),
            (
                Some(ContainerSummaryStateEnum::PAUSED),
                ContainerState::Paused,
            ),
            (
                Some(ContainerSummaryStateEnum::RESTARTING),
                ContainerState::Restarting,
            ),
            (
                Some(ContainerSummaryStateEnum::REMOVING),
                ContainerState::Removing,
            ),
            (
                Some(ContainerSummaryStateEnum::EXITED),
                ContainerState::Exited,
            ),
            (Some(ContainerSummaryStateEnum::DEAD), ContainerState::Dead),
            (
                Some(ContainerSummaryStateEnum::STOPPING),
                ContainerState::Stopping,
            ),
            (
                Some(ContainerSummaryStateEnum::EMPTY),
                ContainerState::Unknown,
            ),
            (None, ContainerState::Unknown),
        ] {
            assert_eq!(map_container_state(wire), expected);
        }
    }

    #[test]
    fn container_mapping_keeps_only_approved_fields() {
        let summary = bollard::models::ContainerSummary {
            id: Some("a".repeat(64)),
            names: Some(vec!["/web".to_owned()]),
            image: Some("nginx:latest".to_owned()),
            created: Some(1_700_000_000),
            state: Some(ContainerSummaryStateEnum::RUNNING),
            status: Some("Up 3s".to_owned()),
            command: Some("secret-canary-command".to_owned()),
            labels: Some(
                [("secret-canary-label".to_owned(), "value".to_owned())]
                    .into_iter()
                    .collect(),
            ),
            ports: Some(vec![bollard::models::PortSummary {
                private_port: 80,
                ..Default::default()
            }]),
            mounts: Some(vec![bollard::models::MountPoint {
                source: Some("secret-canary-mount".to_owned()),
                ..Default::default()
            }]),
            ..Default::default()
        };

        let mapped = map_container_summary(&summary).unwrap();
        assert_eq!(mapped.container_id, "a".repeat(64));
        assert_eq!(mapped.display_name, "web");
        assert_eq!(mapped.image_reference, "nginx:latest");
        assert_eq!(mapped.state, ContainerState::Running);
        assert_eq!(mapped.status, "Up 3s");
        assert_eq!(mapped.created_at_seconds, 1_700_000_000);
        let rendered = format!("{mapped:?}");
        assert!(!rendered.contains("secret-canary"));
    }

    #[test]
    fn container_mapping_rejects_incomplete_daemon_rows() {
        let valid = bollard::models::ContainerSummary {
            id: Some("b".repeat(64)),
            names: Some(vec!["/web".to_owned()]),
            image: Some("nginx:latest".to_owned()),
            created: Some(1_700_000_000),
            state: Some(ContainerSummaryStateEnum::RUNNING),
            status: Some("Up".to_owned()),
            ..Default::default()
        };
        assert!(map_container_summary(&valid).is_ok());

        for mutate in [
            (|summary: &mut bollard::models::ContainerSummary| summary.id = None)
                as fn(&mut bollard::models::ContainerSummary),
            (|summary| summary.id = Some("short".to_owned())),
            (|summary| summary.names = None),
            (|summary| summary.names = Some(vec![])),
            (|summary| summary.image = Some("   ".to_owned())),
            (|summary| summary.created = None),
            (|summary| summary.created = Some(-1)),
        ] {
            let mut broken = valid.clone();
            mutate(&mut broken);
            let error = map_container_summary(&broken).err().unwrap();
            assert_eq!(error.kind(), EngineErrorKind::InvalidResponse);
            assert_eq!(error.operation(), EngineOperation::ListContainers);
        }
    }

    #[tokio::test]
    async fn list_containers_pages_through_before_cursors() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::UnixListener,
        };

        let first_id = "a".repeat(64);
        let second_id = "b".repeat(64);
        let socket_path = std::env::temp_dir()
            .join(format!("kontainer-engine-list-{}.sock", std::process::id(),));
        let _ = std::fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path).unwrap();
        let engine = LocalDockerEngine::connect_for_node(LocalEngineOptions {
            socket_path: socket_path.clone(),
            operation_timeout: Duration::from_secs(2),
        })
        .unwrap();
        let first_body = format!(
            r#"[{{"Id":"{first_id}","Names":["/web"],"Image":"nginx:latest","Created":1700000000,"State":"running","Status":"Up 3s","Command":"secret-canary"}},{{"Id":"{second_id}","Names":["/worker"],"Image":"redis:7","Created":1699999900,"State":"exited","Status":"Exited (0)"}}]"#
        );
        let second_body = format!(
            r#"[{{"Id":"{second_id}","Names":["/worker"],"Image":"redis:7","Created":1699999900,"State":"exited","Status":"Exited (0)"}}]"#
        );
        let fixture = tokio::spawn(async move {
            // Each page negotiates (/version) then lists (/containers/json).
            let bodies = [first_body, second_body];
            let mut seen = Vec::new();
            for body in &bodies {
                for expected_endpoint in ["/version", "/containers/json"] {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let mut request = Vec::new();
                    loop {
                        let byte = socket.read_u8().await.unwrap();
                        request.push(byte);
                        if request.ends_with(b"\r\n\r\n") {
                            break;
                        }
                        assert!(request.len() < 16_384);
                    }
                    let request = String::from_utf8(request).unwrap();
                    let request_line = request.lines().next().unwrap().to_owned();
                    if expected_endpoint == "/version" {
                        assert!(request_line.contains("/version"), "{request_line}");
                        let body =
                            r#"{"ApiVersion":"1.47","MinAPIVersion":"1.47","Version":"27.5.1"}"#;
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
                    } else {
                        assert!(request_line.contains("/containers/json"), "{request_line}");
                        assert!(request_line.contains("all=true"), "{request_line}");
                        assert!(request_line.contains("size=false"), "{request_line}");
                        assert!(request_line.contains("limit=2"), "{request_line}");
                        seen.push(request_line.clone());
                        let body = body.clone();
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
                }
            }
            seen
        });

        let first = engine
            .list_containers(
                ContainerPageRequest::new(1, "").unwrap(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(first.containers.len(), 1);
        assert_eq!(first.containers[0].container_id, first_id);
        assert_eq!(first.containers[0].display_name, "web");
        let cursor = first.next_page_token.clone().unwrap();
        assert_eq!(cursor, format!("v1:{first_id}"));

        let second = engine
            .list_containers(
                ContainerPageRequest::new(1, &cursor).unwrap(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(second.containers.len(), 1);
        assert_eq!(second.containers[0].container_id, second_id);
        assert!(second.next_page_token.is_none());

        let seen = tokio::time::timeout(Duration::from_secs(5), fixture)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(seen.len(), 2);
        assert!(!seen[0].contains("before"));
        assert!(
            seen[1].contains("before"),
            "second page must carry the cursor"
        );
        assert!(seen[1].contains(&first_id));
        std::fs::remove_file(socket_path).unwrap();
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
