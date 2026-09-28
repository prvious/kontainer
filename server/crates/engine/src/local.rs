use std::{collections::HashMap, future::Future, time::Duration};

use async_trait::async_trait;
use bollard::{
    API_DEFAULT_VERSION, Docker,
    errors::Error as BollardError,
    models::{
        ContainerCpuStats, ContainerStatsResponse, ContainerSummaryStateEnum, LocalNodeState,
        SwarmInfo, SystemDataUsageResponse, SystemInfo,
    },
};
use futures_util::StreamExt;
use tokio::time;
use tokio_util::sync::CancellationToken;

use crate::{
    CapabilityStatus, ContainerAction, ContainerActionOptions, ContainerEngine, ContainerPage,
    ContainerPageRequest, ContainerState, ContainerStats, ContainerSummary, ContainerTimes,
    CpuCounters, DEFAULT_STOP_TIMEOUT_SECONDS, DiskUsage, DockerHost, EngineAccessScope,
    EngineApiVersion, EngineCapabilities, EngineDescription, EngineError, EngineErrorKind,
    EngineOperation, NodeInfo, SERVICE_NAME_LABEL, STACK_NAMESPACE_LABEL, TASK_ID_LABEL,
    UnsupportedCapabilityReason, bounded_label, canonical_display_name, cpu_percent,
    format_page_token, is_valid_container_id, memory_used, parse_docker_time, truncate_status_text,
};

const DEFAULT_OPERATION_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalEngineOptions {
    pub host: DockerHost,
    pub operation_timeout: Duration,
}

impl Default for LocalEngineOptions {
    fn default() -> Self {
        Self {
            host: DockerHost::default(),
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

        let bollard_timeout = options.operation_timeout.as_secs().max(1);
        let client = match &options.host {
            DockerHost::UnixSocket(socket_path) => {
                let socket_path = socket_path.to_str().ok_or_else(|| {
                    EngineError::new(
                        EngineErrorKind::InvalidConfiguration,
                        EngineOperation::Connect,
                    )
                })?;
                Docker::connect_with_socket(socket_path, bollard_timeout, API_DEFAULT_VERSION)
            }
            DockerHost::Tcp { .. } => Docker::connect_with_http(
                &options.host.to_string(),
                bollard_timeout,
                API_DEFAULT_VERSION,
            ),
        }
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

    /// [`Self::execute`] for calls that wait out a stop grace: the deadline extends by it.
    async fn execute_with_grace<T, F>(
        &self,
        operation: EngineOperation,
        cancellation: &CancellationToken,
        grace_seconds: i32,
        future: F,
    ) -> Result<T, EngineError>
    where
        F: Future<Output = Result<T, BollardError>>,
    {
        let deadline =
            self.operation_timeout + Duration::from_secs(u64::try_from(grace_seconds).unwrap_or(0));
        tokio::select! {
            biased;
            () = cancellation.cancelled() => Err(EngineError::new(EngineErrorKind::Cancelled, operation)),
            result = time::timeout(deadline, future) => match result {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(error)) => Err(EngineError::from_bollard(operation, &error)),
                Err(_) => Err(EngineError::new(EngineErrorKind::DeadlineExceeded, operation)),
            },
        }
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

    async fn node_info(&self, cancellation: &CancellationToken) -> Result<NodeInfo, EngineError> {
        let client = self.negotiate(cancellation).await?;
        let info = self
            .execute(EngineOperation::ReadSystemInfo, cancellation, client.info())
            .await?;
        Ok(map_node_info(info))
    }

    async fn running_container_ids(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<String>, EngineError> {
        use bollard::query_parameters::ListContainersOptionsBuilder;

        let client = self.negotiate(cancellation).await?;
        let filters = HashMap::from([("status".to_owned(), vec!["running".to_owned()])]);
        let options = ListContainersOptionsBuilder::new()
            .all(false)
            .size(false)
            .filters(&filters)
            .build();
        let summaries = self
            .execute(
                EngineOperation::ListContainers,
                cancellation,
                client.list_containers(Some(options)),
            )
            .await?;
        Ok(summaries
            .into_iter()
            .filter_map(|summary| summary.id.filter(|id| is_valid_container_id(id)))
            .collect())
    }

    async fn container_stats(
        &self,
        container_id: &str,
        cancellation: &CancellationToken,
    ) -> Result<ContainerStats, EngineError> {
        use bollard::query_parameters::StatsOptionsBuilder;

        if !is_valid_container_id(container_id) {
            return Err(EngineError::new(
                EngineErrorKind::InvalidInput,
                EngineOperation::ReadContainerStats,
            ));
        }
        let client = self.negotiate(cancellation).await?;
        // Not one-shot: the daemon waits for a second reading so `precpu_stats` is populated.
        let options = StatsOptionsBuilder::default()
            .stream(false)
            .one_shot(false)
            .build();
        let mut stream = client.stats(container_id, Some(options));
        let response = self
            .execute(EngineOperation::ReadContainerStats, cancellation, async {
                stream.next().await.transpose()
            })
            .await?
            .ok_or_else(|| {
                EngineError::new(
                    EngineErrorKind::InvalidResponse,
                    EngineOperation::ReadContainerStats,
                )
            })?;
        Ok(map_container_stats(response))
    }

    async fn container_times(
        &self,
        container_id: &str,
        cancellation: &CancellationToken,
    ) -> Result<ContainerTimes, EngineError> {
        if !is_valid_container_id(container_id) {
            return Err(EngineError::new(
                EngineErrorKind::InvalidInput,
                EngineOperation::InspectContainer,
            ));
        }
        let client = self.negotiate(cancellation).await?;
        let inspect = self
            .execute(
                EngineOperation::InspectContainer,
                cancellation,
                client.inspect_container(
                    container_id,
                    None::<bollard::query_parameters::InspectContainerOptions>,
                ),
            )
            .await?;
        let state = inspect.state.unwrap_or_default();
        Ok(ContainerTimes {
            started_at_seconds: state.started_at.as_deref().and_then(parse_docker_time),
            finished_at_seconds: state.finished_at.as_deref().and_then(parse_docker_time),
        })
    }

    async fn container_action(
        &self,
        container_id: &str,
        action: ContainerAction,
        options: ContainerActionOptions,
        cancellation: &CancellationToken,
    ) -> Result<(), EngineError> {
        use bollard::query_parameters::{
            RemoveContainerOptionsBuilder, RestartContainerOptionsBuilder,
            StopContainerOptionsBuilder,
        };

        if !is_valid_container_id(container_id) {
            return Err(EngineError::new(
                EngineErrorKind::InvalidInput,
                EngineOperation::ContainerAction,
            ));
        }
        let client = self.negotiate(cancellation).await?;
        let grace = i32::try_from(
            options
                .timeout_seconds
                .unwrap_or(DEFAULT_STOP_TIMEOUT_SECONDS),
        )
        .unwrap_or(i32::MAX);
        let operation = EngineOperation::ContainerAction;
        match action {
            ContainerAction::Start => {
                self.execute(
                    operation,
                    cancellation,
                    client.start_container(
                        container_id,
                        None::<bollard::query_parameters::StartContainerOptions>,
                    ),
                )
                .await
            }
            ContainerAction::Stop => {
                let options = StopContainerOptionsBuilder::default().t(grace).build();
                self.execute_with_grace(
                    operation,
                    cancellation,
                    grace,
                    client.stop_container(container_id, Some(options)),
                )
                .await
            }
            ContainerAction::Restart => {
                let options = RestartContainerOptionsBuilder::default().t(grace).build();
                self.execute_with_grace(
                    operation,
                    cancellation,
                    grace,
                    client.restart_container(container_id, Some(options)),
                )
                .await
            }
            ContainerAction::Pause => {
                self.execute(
                    operation,
                    cancellation,
                    client.pause_container(container_id),
                )
                .await
            }
            ContainerAction::Unpause => {
                self.execute(
                    operation,
                    cancellation,
                    client.unpause_container(container_id),
                )
                .await
            }
            ContainerAction::Remove => {
                let options = RemoveContainerOptionsBuilder::default()
                    .force(options.force)
                    .v(false)
                    .build();
                self.execute(
                    operation,
                    cancellation,
                    client.remove_container(container_id, Some(options)),
                )
                .await
            }
        }
    }

    async fn disk_usage(&self, cancellation: &CancellationToken) -> Result<DiskUsage, EngineError> {
        let client = self.negotiate(cancellation).await?;
        let usage = self
            .execute(
                EngineOperation::ReadDiskUsage,
                cancellation,
                client.df(None::<bollard::query_parameters::DataUsageOptions>),
            )
            .await?;
        Ok(map_disk_usage(&usage))
    }
}

fn map_node_info(info: SystemInfo) -> NodeInfo {
    let count = |value: Option<i64>| {
        value
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0)
    };
    NodeInfo {
        hostname: info.name.unwrap_or_default(),
        swarm_node_id: info
            .swarm
            .and_then(|swarm| swarm.node_id)
            .filter(|id| !id.is_empty()),
        docker_version: info.server_version.unwrap_or_default(),
        os: info.operating_system.or(info.os_type).unwrap_or_default(),
        architecture: info.architecture.unwrap_or_default(),
        cpu_cores: count(info.ncpu),
        memory_total_bytes: info
            .mem_total
            .and_then(|value| u64::try_from(value).ok())
            .unwrap_or(0),
        containers_running: count(info.containers_running),
        containers_paused: count(info.containers_paused),
        containers_stopped: count(info.containers_stopped),
    }
}

fn cpu_counters(stats: Option<&ContainerCpuStats>) -> CpuCounters {
    stats.map_or_else(CpuCounters::default, |stats| CpuCounters {
        total_usage: stats
            .cpu_usage
            .as_ref()
            .and_then(|usage| usage.total_usage)
            .unwrap_or(0),
        system_usage: stats.system_cpu_usage.unwrap_or(0),
        online_cpus: stats.online_cpus.unwrap_or(0),
    })
}

fn map_container_stats(response: ContainerStatsResponse) -> ContainerStats {
    let memory = response.memory_stats.unwrap_or_default();
    let (net_rx_bytes, net_tx_bytes) =
        response
            .networks
            .unwrap_or_default()
            .values()
            .fold((0_u64, 0_u64), |(rx, tx), network| {
                (
                    rx.saturating_add(network.rx_bytes.unwrap_or(0)),
                    tx.saturating_add(network.tx_bytes.unwrap_or(0)),
                )
            });
    ContainerStats {
        cpu_percent: cpu_percent(
            cpu_counters(response.cpu_stats.as_ref()),
            cpu_counters(response.precpu_stats.as_ref()),
        ),
        memory_used_bytes: memory
            .usage
            .map(|usage| memory_used(usage, &memory.stats.clone().unwrap_or_default())),
        memory_limit_bytes: memory.limit.filter(|limit| *limit > 0),
        net_rx_bytes,
        net_tx_bytes,
    }
}

fn map_disk_usage(usage: &SystemDataUsageResponse) -> DiskUsage {
    let totals = [
        usage
            .image_usage
            .as_ref()
            .and_then(|usage| usage.total_size),
        usage
            .container_usage
            .as_ref()
            .and_then(|usage| usage.total_size),
        usage
            .volume_usage
            .as_ref()
            .and_then(|usage| usage.total_size),
        usage
            .build_cache_usage
            .as_ref()
            .and_then(|usage| usage.total_size),
    ];
    let reported: Vec<u64> = totals
        .into_iter()
        .flatten()
        .filter_map(|size| u64::try_from(size).ok())
        .collect();
    DiskUsage {
        total_bytes: (!reported.is_empty()).then(|| reported.iter().sum()),
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

    let labels = summary.labels.clone().unwrap_or_default();
    Ok(ContainerSummary {
        container_id,
        display_name,
        image_reference,
        state: map_container_state(summary.state),
        status: truncate_status_text(summary.status.as_deref().unwrap_or_default()),
        created_at_seconds,
        published_ports: published_ports(summary.ports.as_deref().unwrap_or_default()),
        service_name: bounded_label(labels.get(SERVICE_NAME_LABEL)),
        stack_namespace: bounded_label(labels.get(STACK_NAMESPACE_LABEL)),
        task_id: bounded_label(labels.get(TASK_ID_LABEL)),
    })
}

/// `host:container/proto` for each published port, once each (IPv4 and IPv6 bindings of one port
/// collapse), sorted.
fn published_ports(ports: &[bollard::models::PortSummary]) -> Vec<String> {
    let mut published: Vec<String> = ports
        .iter()
        .filter_map(|port| {
            let public = port.public_port?;
            let protocol = port.typ.map_or("tcp", |typ| match typ {
                bollard::models::PortSummaryTypeEnum::UDP => "udp",
                bollard::models::PortSummaryTypeEnum::SCTP => "sctp",
                _ => "tcp",
            });
            Some(format!("{public}:{}/{protocol}", port.private_port))
        })
        .collect();
    published.sort();
    published.dedup();
    published
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
        map_container_stats, map_container_summary, map_disk_usage, map_node_info,
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
                host: crate::DockerHost::UnixSocket(missing_socket.clone()),
                operation_timeout: Duration::from_secs(1),
            })
            .err()
            .unwrap();
            assert_eq!(error.kind(), EngineErrorKind::ConnectionUnavailable);
            assert_eq!(error.operation(), EngineOperation::Connect);
        }

        assert_ne!(EngineAccessScope::Manager, EngineAccessScope::NodeLocal);
    }

    #[tokio::test]
    async fn tcp_hosts_connect_over_http_and_map_refusal_to_unavailable() {
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = closed.local_addr().unwrap().port();
        drop(closed);
        let engine = LocalDockerEngine::connect_for_node(LocalEngineOptions {
            host: format!("tcp://127.0.0.1:{port}").parse().unwrap(),
            operation_timeout: Duration::from_secs(1),
        })
        .unwrap();

        let error = engine.is_available().await.unwrap_err();
        assert_eq!(error.kind(), EngineErrorKind::ConnectionUnavailable);
    }

    #[test]
    fn zero_timeout_is_rejected_before_adapter_construction() {
        let error = LocalDockerEngine::connect_for_node(LocalEngineOptions {
            host: crate::DockerHost::default(),
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
            host: crate::DockerHost::UnixSocket(socket_path.clone()),
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
            host: crate::DockerHost::UnixSocket(socket_path.clone()),
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

    #[tokio::test]
    async fn removed_cursor_is_invalid_pagination() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::UnixListener,
        };

        let socket_path = std::env::temp_dir().join(format!(
            "kontainer-engine-stale-cursor-{}.sock",
            std::process::id(),
        ));
        let _ = std::fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path).unwrap();
        let engine = LocalDockerEngine::connect_for_node(LocalEngineOptions {
            host: crate::DockerHost::UnixSocket(socket_path.clone()),
            operation_timeout: Duration::from_secs(2),
        })
        .unwrap();
        let fixture = tokio::spawn(async move {
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
                if expected_endpoint == "/version" {
                    let body = r#"{"ApiVersion":"1.47","MinAPIVersion":"1.47","Version":"27.5.1"}"#;
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
                    let body = r#"{"message":"No such container"}"#;
                    socket
                        .write_all(
                            format!(
                                "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                                body.len(),
                            )
                            .as_bytes(),
                        )
                        .await
                        .unwrap();
                }
            }
        });

        let stale = format!("v1:{}", "c".repeat(64));
        let error = engine
            .list_containers(
                ContainerPageRequest::new(10, &stale).unwrap(),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.kind(), EngineErrorKind::InvalidInput);
        assert_eq!(error.operation(), EngineOperation::ListContainers);
        tokio::time::timeout(Duration::from_secs(5), fixture)
            .await
            .unwrap()
            .unwrap();
        std::fs::remove_file(socket_path).unwrap();
    }

    #[test]
    fn summaries_carry_published_ports_and_the_allowlisted_labels_only() {
        use bollard::models::{PortSummary, PortSummaryTypeEnum};
        let summary = bollard::models::ContainerSummary {
            id: Some("a".repeat(64)),
            names: Some(vec!["/web.1.abc".to_owned()]),
            image: Some("nginx:latest".to_owned()),
            created: Some(1_700_000_000),
            state: Some(ContainerSummaryStateEnum::RUNNING),
            status: Some("Up".to_owned()),
            ports: Some(vec![
                PortSummary {
                    private_port: 80,
                    public_port: Some(8080),
                    typ: Some(PortSummaryTypeEnum::TCP),
                    ip: Some("0.0.0.0".to_owned()),
                },
                PortSummary {
                    private_port: 80,
                    public_port: Some(8080),
                    typ: Some(PortSummaryTypeEnum::TCP),
                    ip: Some("::".to_owned()),
                },
                PortSummary {
                    private_port: 53,
                    public_port: Some(5353),
                    typ: Some(PortSummaryTypeEnum::UDP),
                    ip: None,
                },
                PortSummary {
                    private_port: 9000,
                    public_port: None,
                    typ: Some(PortSummaryTypeEnum::TCP),
                    ip: None,
                },
            ]),
            labels: Some(
                [
                    ("com.docker.swarm.service.name", "shop_web"),
                    ("com.docker.stack.namespace", "shop"),
                    ("com.docker.swarm.task.id", "t1"),
                    ("secret-canary-label", "value"),
                ]
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect(),
            ),
            ..Default::default()
        };
        let mapped = map_container_summary(&summary).unwrap();
        assert_eq!(mapped.published_ports, vec!["5353:53/udp", "8080:80/tcp"]);
        assert_eq!(mapped.service_name.as_deref(), Some("shop_web"));
        assert_eq!(mapped.stack_namespace.as_deref(), Some("shop"));
        assert_eq!(mapped.task_id.as_deref(), Some("t1"));
        assert!(!format!("{mapped:?}").contains("secret-canary"));
    }

    #[test]
    fn daemon_statuses_map_to_lifecycle_kinds() {
        use crate::EngineError;
        let status = |status_code| bollard::errors::Error::DockerResponseServerError {
            status_code,
            message: "secret-canary".to_owned(),
        };
        for (code, kind) in [
            (404, EngineErrorKind::NotFound),
            (304, EngineErrorKind::NotModified),
            (409, EngineErrorKind::Conflict),
            (500, EngineErrorKind::DaemonRejected),
        ] {
            let error = EngineError::from_bollard(EngineOperation::ContainerAction, &status(code));
            assert_eq!(error.kind(), kind, "{code}");
            assert!(!error.to_string().contains("secret-canary"));
        }
        assert_eq!(
            EngineError::from_bollard(EngineOperation::ListContainers, &status(404)).kind(),
            EngineErrorKind::InvalidInput,
            "a stale cursor stays invalid pagination"
        );
    }

    #[test]
    fn stats_mapping_matches_docker_stats_math() {
        use bollard::models::{
            ContainerCpuStats, ContainerCpuUsage, ContainerMemoryStats, ContainerNetworkStats,
            ContainerStatsResponse,
        };

        let cpu = |total_usage, system_cpu_usage| ContainerCpuStats {
            cpu_usage: Some(ContainerCpuUsage {
                total_usage: Some(total_usage),
                ..Default::default()
            }),
            system_cpu_usage: Some(system_cpu_usage),
            online_cpus: Some(2),
            ..Default::default()
        };
        let network = |rx, tx| ContainerNetworkStats {
            rx_bytes: Some(rx),
            tx_bytes: Some(tx),
            ..Default::default()
        };
        let response = ContainerStatsResponse {
            cpu_stats: Some(cpu(3_000_000, 40_000_000)),
            precpu_stats: Some(cpu(1_000_000, 20_000_000)),
            memory_stats: Some(ContainerMemoryStats {
                usage: Some(5000),
                limit: Some(100_000),
                stats: Some([("inactive_file".to_owned(), 1000)].into_iter().collect()),
                ..Default::default()
            }),
            networks: Some(
                [
                    ("eth0".to_owned(), network(100, 10)),
                    ("eth1".to_owned(), network(50, 5)),
                ]
                .into_iter()
                .collect(),
            ),
            ..Default::default()
        };

        let stats = map_container_stats(response);
        assert_eq!(stats.cpu_percent, Some(20.0));
        assert_eq!(stats.memory_used_bytes, Some(4000));
        assert_eq!(stats.memory_limit_bytes, Some(100_000));
        assert_eq!((stats.net_rx_bytes, stats.net_tx_bytes), (150, 15));

        let empty = map_container_stats(ContainerStatsResponse::default());
        assert_eq!(empty.cpu_percent, None);
        assert_eq!(empty.memory_used_bytes, None);
        assert_eq!(empty.memory_limit_bytes, None);
    }

    #[test]
    fn disk_usage_sums_reported_categories_and_is_absent_otherwise() {
        use bollard::models::{
            ContainersDiskUsage, ImagesDiskUsage, SystemDataUsageResponse, VolumesDiskUsage,
        };

        let usage = SystemDataUsageResponse {
            image_usage: Some(ImagesDiskUsage {
                total_size: Some(1000),
                ..Default::default()
            }),
            container_usage: Some(ContainersDiskUsage {
                total_size: Some(24),
                ..Default::default()
            }),
            volume_usage: Some(VolumesDiskUsage::default()),
            build_cache_usage: None,
        };
        assert_eq!(map_disk_usage(&usage).total_bytes, Some(1024));
        assert_eq!(
            map_disk_usage(&SystemDataUsageResponse::default()).total_bytes,
            None
        );
    }

    #[test]
    fn node_info_keeps_counts_and_drops_an_empty_swarm_id() {
        let info = bollard::models::SystemInfo {
            name: Some("manager".to_owned()),
            ncpu: Some(4),
            mem_total: Some(8 << 30),
            operating_system: Some("Alpine Linux".to_owned()),
            architecture: Some("aarch64".to_owned()),
            server_version: Some("29.8.0".to_owned()),
            containers_running: Some(5),
            containers_paused: Some(1),
            containers_stopped: Some(2),
            swarm: Some(SwarmInfo {
                node_id: Some(String::new()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let node = map_node_info(info);
        assert_eq!(node.hostname, "manager");
        assert_eq!(node.cpu_cores, 4);
        assert_eq!(node.memory_total_bytes, 8 << 30);
        assert_eq!(
            (
                node.containers_running,
                node.containers_paused,
                node.containers_stopped
            ),
            (5, 1, 2)
        );
        assert_eq!(node.swarm_node_id, None);
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
