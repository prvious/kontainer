#![forbid(unsafe_code)]

//! Production controller-to-agent client for one node.
//!
//! This is the only production path that dials a node agent. Membership
//! (`nodes::agents`) builds one per node from its advertised address. It calls
//! the agent with explicit connect and RPC deadlines, verifies the peer
//! workload identity during TLS, cross-checks the reported node ID, and
//! propagates caller and shutdown cancellation without spawning detached
//! work. There is no retry, pooling, or channel management here.

use std::time::Duration;

use kontainer_agent_proto::v1::{
    ContainerAction as ProtoContainerAction, ContainerActionRequest,
    ContainerState as ProtoContainerState, DescribeRequest, ListContainersRequest,
    ListImagesRequest, PullImageRequest, RemoveImageRequest, node_agent_client::NodeAgentClient,
};
use kontainer_agent_proto::v1::{
    CreateNetworkRequest, InspectNetworkRequest, ListNetworksRequest,
    NetworkSummary as ProtoNetworkSummary, RemoveNetworkRequest,
};
use kontainer_agent_proto::v1::{
    CreateVolumeRequest, ListVolumesRequest, RemoveVolumeRequest,
    VolumeSummary as ProtoVolumeSummary,
};
use kontainer_agent_proto::v1::{
    ExecClientFrame, ExecCloseRequest, ExecOpenRequest, ExecResizeRequest, exec_client_frame,
    exec_server_frame,
};
use kontainer_rpc_auth::{MutualTlsIdentityFiles, WorkloadIdentity};
use tokio_util::sync::CancellationToken;
use tonic::transport::{Channel, Endpoint};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const RPC_TIMEOUT: Duration = Duration::from_secs(2);

/// Validated static configuration for the single agent endpoint.
#[derive(Debug, Clone)]
pub struct NodeAgentClientConfig {
    endpoint: String,
    server_name: String,
    expected: WorkloadIdentity,
    tls_files: MutualTlsIdentityFiles,
    connect_timeout: Duration,
    rpc_timeout: Duration,
}

impl NodeAgentClientConfig {
    /// Validate static settings and TLS material without connecting.
    ///
    /// Network unavailability must not fail here; only malformed settings
    /// and unreadable or unparsable TLS files fail. Building the Tonic
    /// endpoint parses the certificate material, so mismatched keys fail
    /// here rather than on the first call.
    pub fn new(
        endpoint: String,
        server_name: String,
        cluster_id: String,
        node_id: String,
        tls_files: MutualTlsIdentityFiles,
    ) -> Result<Self, AgentClientConfigError> {
        Self::with_timeouts(
            endpoint,
            server_name,
            cluster_id,
            node_id,
            tls_files,
            CONNECT_TIMEOUT,
            RPC_TIMEOUT,
        )
    }

    fn with_timeouts(
        endpoint: String,
        server_name: String,
        cluster_id: String,
        node_id: String,
        tls_files: MutualTlsIdentityFiles,
        connect_timeout: Duration,
        rpc_timeout: Duration,
    ) -> Result<Self, AgentClientConfigError> {
        validate_endpoint(&endpoint)?;
        validate_server_name(&server_name)?;
        validate_identity_segment(&cluster_id, "cluster ID")?;
        validate_identity_segment(&node_id, "node ID")?;

        let expected = WorkloadIdentity::NodeAgent {
            cluster_id,
            node_id,
        };

        let tls_config = tls_files
            .client_config_for_custom_verifier(server_name.clone())
            .map_err(|error| {
                AgentClientConfigError::InvalidTls(format!(
                    "agent client TLS files are invalid: {error}"
                ))
            })?;
        let verifier = tls_files
            .outbound_verifier(expected.clone())
            .map_err(|error| {
                AgentClientConfigError::InvalidTls(format!(
                    "agent client TLS files are invalid: {error}"
                ))
            })?;

        // Eagerly parse certificate material. This does not connect; an
        // unreachable agent still constructs successfully and fails only
        // when `describe` is called.
        Endpoint::from_shared(endpoint.clone())
            .map_err(|_| AgentClientConfigError::InvalidEndpoint)?
            .connect_timeout(connect_timeout)
            .timeout(rpc_timeout)
            .tls_config_with_verifier(tls_config, verifier)
            .map_err(|_| {
                AgentClientConfigError::InvalidTls("agent client TLS files are invalid".to_owned())
            })?;

        Ok(Self {
            endpoint,
            server_name,
            expected,
            tls_files,
            connect_timeout,
            rpc_timeout,
        })
    }

    fn endpoint_prototype(&self, rpc_timeout: Duration) -> Result<Endpoint, AgentClientError> {
        let tls_config = self
            .tls_files
            .client_config_for_custom_verifier(self.server_name.clone())
            .map_err(|_| AgentClientError::Unavailable)?;
        let verifier = self
            .tls_files
            .outbound_verifier(self.expected.clone())
            .map_err(|_| AgentClientError::Unavailable)?;
        // Deliberately no `Endpoint::connect_timeout` here: tonic's connector
        // timeout surfaces as a transport error and would map a connect
        // deadline expiry to `Unavailable` (503) instead of `TimedOut` (504).
        // The outer deadline in `channel` is the single connect deadline.
        Endpoint::from_shared(self.endpoint.clone())
            .map_err(|_| AgentClientError::Unavailable)?
            .timeout(rpc_timeout)
            .tls_config_with_verifier(tls_config, verifier)
            .map_err(|_| AgentClientError::Unavailable)
    }

    /// Call `Describe` on the statically configured agent.
    ///
    /// The endpoint, DNS name, and expected identity come only from this
    /// client's configuration. No request input selects or overrides them.
    pub async fn describe(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<AgentDescription, AgentClientError> {
        let channel = self.channel(cancellation).await?;

        let mut client = NodeAgentClient::new(channel);
        let mut request = tonic::Request::new(DescribeRequest {});
        request.set_timeout(self.rpc_timeout);

        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(self.rpc_timeout, client.describe(request)) => {
                match result {
                    Ok(Ok(response)) => response.into_inner(),
                    Ok(Err(status)) => return Err(map_rpc_status(status)),
                    Err(_) => return Err(AgentClientError::TimedOut),
                }
            }
        };

        let expected_node = self
            .expected
            .node_id()
            .ok_or(AgentClientError::InvalidResponse)?;
        if response.node_id != expected_node {
            return Err(AgentClientError::IdentityMismatch);
        }

        Ok(AgentDescription {
            node_id: response.node_id,
            agent_version: response.agent_version,
            architecture: response.architecture,
            docker_connected: response.docker_connected,
        })
    }

    /// Call `ListContainers` on the statically configured agent.
    ///
    /// Only the page size and opaque page token vary per call. The endpoint,
    /// DNS name, and expected identity come only from this client's
    /// configuration. The returned page preserves the agent's deterministic
    /// order and cursor unchanged.
    pub async fn list_containers(
        &self,
        page_size: u32,
        page_token: String,
        cancellation: &CancellationToken,
    ) -> Result<AgentContainerPage, AgentClientError> {
        let channel = self.channel(cancellation).await?;

        let mut client = NodeAgentClient::new(channel);
        let mut request = tonic::Request::new(ListContainersRequest {
            page_size,
            page_token,
        });
        request.set_timeout(self.rpc_timeout);

        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(self.rpc_timeout, client.list_containers(request)) => {
                match result {
                    Ok(Ok(response)) => response.into_inner(),
                    Ok(Err(status)) => return Err(map_rpc_status(status)),
                    Err(_) => return Err(AgentClientError::TimedOut),
                }
            }
        };

        self.validate_container_page(page_size, response)
    }

    /// Open the single static channel, observing connect deadlines and
    /// caller/shutdown cancellation without spawning detached work.
    async fn channel(&self, cancellation: &CancellationToken) -> Result<Channel, AgentClientError> {
        self.channel_with_timeout(cancellation, self.rpc_timeout)
            .await
    }

    /// A channel whose per-request timeout is `rpc_timeout` (actions that wait out a stop grace
    /// need longer than reads).
    async fn channel_with_timeout(
        &self,
        cancellation: &CancellationToken,
        rpc_timeout: Duration,
    ) -> Result<Channel, AgentClientError> {
        let endpoint = self.endpoint_prototype(rpc_timeout)?;

        tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(self.connect_timeout, endpoint.connect()) => {
                match result {
                    Ok(Ok(channel)) => Ok(channel),
                    Ok(Err(_)) => Err(AgentClientError::Unavailable),
                    Err(_) => Err(AgentClientError::TimedOut),
                }
            }
        }
    }

    /// Validate one agent inventory page against the configured identity and
    /// the #82 bounds. Order is preserved; any violation is a redacted
    /// invalid response rather than a partial page.
    fn validate_container_page(
        &self,
        page_size: u32,
        response: kontainer_agent_proto::v1::ListContainersResponse,
    ) -> Result<AgentContainerPage, AgentClientError> {
        let expected_node = self
            .expected
            .node_id()
            .ok_or(AgentClientError::InvalidResponse)?;
        let limit = usize::try_from(page_size).unwrap_or(MAX_CONTAINER_PAGE_SIZE);
        // The controller always sends an explicit 1..=100 size; zero would
        // mean the agent default, so bound it by the hard maximum instead of
        // trusting an unbounded page.
        let bound = if limit == 0 {
            MAX_CONTAINER_PAGE_SIZE
        } else {
            limit
        };
        if response.containers.len() > bound || response.containers.len() > MAX_CONTAINER_PAGE_SIZE
        {
            return Err(AgentClientError::InvalidResponse);
        }
        if !response.next_page_token.is_empty() && !is_valid_page_token(&response.next_page_token) {
            return Err(AgentClientError::InvalidResponse);
        }

        let mut containers = Vec::with_capacity(response.containers.len());
        for item in response.containers {
            let state = validate_summary(&item, expected_node)?;
            containers.push(AgentContainerSummary {
                container_id: item.container_id,
                display_name: item.display_name,
                image_reference: item.image_reference,
                state,
                status: item.status,
                created_at_seconds: item.created_at_unix_seconds,
                node_id: item.node_id,
                started_at_seconds: item.started_at_unix_seconds.filter(|at| *at > 0),
                finished_at_seconds: item.finished_at_unix_seconds.filter(|at| *at > 0),
                published_ports: item.published_ports,
                service_name: item.service_name,
                stack_namespace: item.stack_namespace,
                task_id: item.task_id,
                cpu_percent: item.cpu_percent,
                memory_used_bytes: item.memory_used_bytes,
                memory_limit_bytes: item.memory_limit_bytes,
                networks: item.networks,
            });
        }

        Ok(AgentContainerPage {
            containers,
            next_page_token: if response.next_page_token.is_empty() {
                None
            } else {
                Some(response.next_page_token)
            },
        })
    }

    /// Run one lifecycle action on a container of this node.
    pub async fn container_action(
        &self,
        container_id: &str,
        action: AgentContainerAction,
        force: bool,
        timeout_seconds: Option<u32>,
        cancellation: &CancellationToken,
    ) -> Result<(), AgentClientError> {
        if !is_valid_container_id(container_id) {
            return Err(AgentClientError::InvalidResponse);
        }
        // Stop and restart wait out their grace on the node; the deadline covers it.
        let deadline = self.rpc_timeout
            + Duration::from_secs(u64::from(timeout_seconds.unwrap_or(10)))
            + ACTION_ALLOWANCE;
        let channel = self.channel_with_timeout(cancellation, deadline).await?;
        let mut client = NodeAgentClient::new(channel);
        let mut request = tonic::Request::new(ContainerActionRequest {
            container_id: container_id.to_owned(),
            action: action.to_proto() as i32,
            force,
            timeout_seconds,
        });
        request.set_timeout(deadline);
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(deadline, client.container_action(request)) => match result {
                Ok(Ok(_)) => Ok(()),
                Ok(Err(status)) => Err(map_action_status(&status)),
                Err(_) => Err(AgentClientError::TimedOut),
            }
        }
    }

    /// Every image on the node.
    pub async fn list_images(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<AgentImage>, AgentClientError> {
        let channel = self.channel(cancellation).await?;
        let mut client = NodeAgentClient::new(channel);
        let mut request = tonic::Request::new(ListImagesRequest {});
        request.set_timeout(self.rpc_timeout);
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(self.rpc_timeout, client.list_images(request)) => match result {
                Ok(Ok(response)) => response.into_inner(),
                Ok(Err(status)) => return Err(map_rpc_status(status)),
                Err(_) => return Err(AgentClientError::TimedOut),
            }
        };
        if response.images.len() > MAX_IMAGES_PER_NODE {
            return Err(AgentClientError::InvalidResponse);
        }
        response
            .images
            .into_iter()
            .map(|image| {
                let text_ok = |values: &[String]| {
                    values.len() <= 64 && values.iter().all(|value| value.len() <= 512)
                };
                if !is_valid_image_id(&image.image_id)
                    || !text_ok(&image.repo_tags)
                    || !text_ok(&image.repo_digests)
                {
                    return Err(AgentClientError::InvalidResponse);
                }
                Ok(AgentImage {
                    image_id: image.image_id,
                    repo_tags: image.repo_tags,
                    repo_digests: image.repo_digests,
                    size_bytes: image.size_bytes,
                    created_at_seconds: image.created_at_unix_seconds.max(0),
                    used_by: image.used_by,
                    dangling: image.dangling,
                })
            })
            .collect()
    }

    /// Remove one image from this node.
    pub async fn remove_image(
        &self,
        image_id: &str,
        force: bool,
        cancellation: &CancellationToken,
    ) -> Result<(), AgentClientError> {
        if !is_valid_image_id(image_id) {
            return Err(AgentClientError::InvalidResponse);
        }
        let deadline = self.rpc_timeout + ACTION_ALLOWANCE;
        let channel = self.channel_with_timeout(cancellation, deadline).await?;
        let mut client = NodeAgentClient::new(channel);
        let mut request = tonic::Request::new(RemoveImageRequest {
            image_id: image_id.to_owned(),
            force,
        });
        request.set_timeout(deadline);
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(deadline, client.remove_image(request)) => match result {
                Ok(Ok(_)) => Ok(()),
                Ok(Err(status)) => Err(map_action_status(&status)),
                Err(_) => Err(AgentClientError::TimedOut),
            }
        }
    }

    /// Start an anonymous pull on this node; events end with the pull or one failure.
    /// Open a TTY shell on the agent. `input` carries what follows the open; the stream ends
    /// after the exit or an error, or when the connection drops.
    pub async fn exec(
        &self,
        open: AgentExecOpen,
        input: tokio::sync::mpsc::Receiver<AgentExecInput>,
        cancellation: &CancellationToken,
    ) -> Result<AgentExecStream, AgentClientError> {
        use futures_util::StreamExt as _;

        let channel = self
            .channel_with_timeout(cancellation, EXEC_DEADLINE)
            .await?;
        let mut client = NodeAgentClient::new(channel);
        let first = ExecClientFrame {
            frame: Some(exec_client_frame::Frame::Open(ExecOpenRequest {
                container_id: open.container_id,
                shell: open.shell,
                cols: open.cols,
                rows: open.rows,
            })),
        };
        let outbound = futures_util::stream::once(std::future::ready(first))
            .chain(tokio_stream::wrappers::ReceiverStream::new(input).map(exec_frame));
        let mut request = tonic::Request::new(outbound);
        request.set_timeout(EXEC_DEADLINE);
        let streaming = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(self.rpc_timeout + ACTION_ALLOWANCE, client.exec(request)) => match result {
                Ok(Ok(response)) => response.into_inner(),
                Ok(Err(status)) => return Err(map_action_status(&status)),
                Err(_) => return Err(AgentClientError::TimedOut),
            }
        };
        Ok(Box::pin(futures_util::stream::unfold(
            Some(streaming),
            |state| async move {
                let mut streaming = state?;
                loop {
                    match streaming.message().await {
                        Ok(Some(message)) => {
                            if let Some(frame) = message.frame {
                                let event = exec_event(frame);
                                let last = matches!(
                                    event,
                                    AgentExecEvent::Exit { .. } | AgentExecEvent::Error { .. }
                                );
                                return Some((event, (!last).then_some(streaming)));
                            }
                        }
                        Ok(None) | Err(_) => return None,
                    }
                }
            },
        )))
    }

    pub async fn pull_image(
        &self,
        reference: &str,
        cancellation: &CancellationToken,
    ) -> Result<AgentPullStream, AgentClientError> {
        let channel = self
            .channel_with_timeout(cancellation, PULL_DEADLINE)
            .await?;
        let mut client = NodeAgentClient::new(channel);
        let mut request = tonic::Request::new(PullImageRequest {
            reference: reference.to_owned(),
        });
        request.set_timeout(PULL_DEADLINE);
        let streaming = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(self.rpc_timeout + ACTION_ALLOWANCE, client.pull_image(request)) => match result {
                Ok(Ok(response)) => response.into_inner(),
                Ok(Err(status)) => return Err(map_action_status(&status)),
                Err(_) => return Err(AgentClientError::TimedOut),
            }
        };
        Ok(Box::pin(futures_util::stream::unfold(
            Some(streaming),
            |state| async move {
                let mut streaming = state?;
                match streaming.message().await {
                    Ok(Some(event)) => {
                        let event = match event.error {
                            Some(message) => AgentPullEvent::Failed(message),
                            None => AgentPullEvent::Progress {
                                layer_id: event.layer_id,
                                status: event.status,
                                current: event.current,
                                total: event.total,
                            },
                        };
                        let next = if matches!(event, AgentPullEvent::Failed(_)) {
                            None
                        } else {
                            Some(streaming)
                        };
                        Some((event, next))
                    }
                    Ok(None) => None,
                    Err(_) => Some((
                        AgentPullEvent::Failed(
                            "the node agent stopped answering during the pull".to_owned(),
                        ),
                        None,
                    )),
                }
            },
        )))
    }

    /// Every volume on the node, with its users and last known size.
    pub async fn list_volumes(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<AgentVolume>, AgentClientError> {
        let channel = self.channel(cancellation).await?;
        let mut client = NodeAgentClient::new(channel);
        let mut request = tonic::Request::new(ListVolumesRequest {});
        request.set_timeout(self.rpc_timeout);
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(self.rpc_timeout, client.list_volumes(request)) => match result {
                Ok(Ok(response)) => response.into_inner(),
                Ok(Err(status)) => return Err(map_rpc_status(status)),
                Err(_) => return Err(AgentClientError::TimedOut),
            }
        };
        if response.volumes.len() > MAX_VOLUMES_PER_NODE {
            return Err(AgentClientError::InvalidResponse);
        }
        response.volumes.into_iter().map(agent_volume).collect()
    }

    /// Create one volume on this node.
    pub async fn create_volume(
        &self,
        volume: &AgentVolumeCreate,
        cancellation: &CancellationToken,
    ) -> Result<AgentVolume, AgentClientError> {
        let deadline = self.rpc_timeout + ACTION_ALLOWANCE;
        let channel = self.channel_with_timeout(cancellation, deadline).await?;
        let mut client = NodeAgentClient::new(channel);
        let mut request = tonic::Request::new(CreateVolumeRequest {
            name: volume.name.clone(),
            driver: volume.driver.clone(),
            labels: volume.labels.clone().into_iter().collect(),
            driver_opts: std::collections::HashMap::new(),
        });
        request.set_timeout(deadline);
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(deadline, client.create_volume(request)) => match result {
                Ok(Ok(response)) => response.into_inner(),
                Ok(Err(status)) => return Err(map_action_status(&status)),
                Err(_) => return Err(AgentClientError::TimedOut),
            }
        };
        response
            .volume
            .ok_or(AgentClientError::InvalidResponse)
            .and_then(agent_volume)
    }

    /// Remove one volume from this node; a mounted volume comes back as [`VolumeRemoval::InUse`].
    pub async fn remove_volume(
        &self,
        name: &str,
        cancellation: &CancellationToken,
    ) -> Result<VolumeRemoval, AgentClientError> {
        let deadline = self.rpc_timeout + ACTION_ALLOWANCE;
        let channel = self.channel_with_timeout(cancellation, deadline).await?;
        let mut client = NodeAgentClient::new(channel);
        let mut request = tonic::Request::new(RemoveVolumeRequest {
            name: name.to_owned(),
        });
        request.set_timeout(deadline);
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(deadline, client.remove_volume(request)) => match result {
                Ok(Ok(response)) => response.into_inner(),
                Ok(Err(status)) => return Err(map_action_status(&status)),
                Err(_) => return Err(AgentClientError::TimedOut),
            }
        };
        if response.in_use_by.len() > MAX_VOLUME_USERS
            || response.in_use_by.iter().any(|user| user.len() > 512)
        {
            return Err(AgentClientError::InvalidResponse);
        }
        Ok(if response.in_use_by.is_empty() {
            VolumeRemoval::Removed
        } else {
            VolumeRemoval::InUse(response.in_use_by)
        })
    }

    /// Every network on the node, with its running endpoints here.
    pub async fn list_networks(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<AgentNetwork>, AgentClientError> {
        let channel = self.channel(cancellation).await?;
        let mut client = NodeAgentClient::new(channel);
        let mut request = tonic::Request::new(ListNetworksRequest {});
        request.set_timeout(self.rpc_timeout);
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(self.rpc_timeout, client.list_networks(request)) => match result {
                Ok(Ok(response)) => response.into_inner(),
                Ok(Err(status)) => return Err(map_rpc_status(status)),
                Err(_) => return Err(AgentClientError::TimedOut),
            }
        };
        if response.networks.len() > MAX_NETWORKS_PER_NODE {
            return Err(AgentClientError::InvalidResponse);
        }
        response.networks.into_iter().map(agent_network).collect()
    }

    /// One network's endpoints on this node.
    pub async fn inspect_network(
        &self,
        id: &str,
        cancellation: &CancellationToken,
    ) -> Result<AgentNetworkDetail, AgentClientError> {
        if !kontainer_engine::is_valid_network_id(id) {
            return Err(AgentClientError::InvalidResponse);
        }
        let channel = self.channel(cancellation).await?;
        let mut client = NodeAgentClient::new(channel);
        let mut request = tonic::Request::new(InspectNetworkRequest { id: id.to_owned() });
        request.set_timeout(self.rpc_timeout);
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(self.rpc_timeout, client.inspect_network(request)) => match result {
                Ok(Ok(response)) => response.into_inner(),
                Ok(Err(status)) => return Err(map_action_status(&status)),
                Err(_) => return Err(AgentClientError::TimedOut),
            }
        };
        if response.endpoints.len() > MAX_NETWORK_ENDPOINTS {
            return Err(AgentClientError::InvalidResponse);
        }
        let network = response
            .network
            .ok_or(AgentClientError::InvalidResponse)
            .and_then(agent_network)?;
        let endpoints = response
            .endpoints
            .into_iter()
            .map(|endpoint| {
                if !is_valid_container_id(&endpoint.container_id) || endpoint.name.len() > 512 {
                    return Err(AgentClientError::InvalidResponse);
                }
                Ok(AgentNetworkEndpoint {
                    container_id: endpoint.container_id,
                    name: endpoint.name,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(AgentNetworkDetail { network, endpoints })
    }

    /// Create a local bridge network on this node and return its id.
    pub async fn create_network(
        &self,
        network: &AgentNetworkCreate,
        cancellation: &CancellationToken,
    ) -> Result<String, AgentClientError> {
        let deadline = self.rpc_timeout + ACTION_ALLOWANCE;
        let channel = self.channel_with_timeout(cancellation, deadline).await?;
        let mut client = NodeAgentClient::new(channel);
        let mut request = tonic::Request::new(CreateNetworkRequest {
            name: network.name.clone(),
            subnet: network.subnet.clone(),
            labels: network.labels.clone().into_iter().collect(),
        });
        request.set_timeout(deadline);
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(deadline, client.create_network(request)) => match result {
                Ok(Ok(response)) => response.into_inner(),
                Ok(Err(status)) => return Err(map_action_status(&status)),
                Err(_) => return Err(AgentClientError::TimedOut),
            }
        };
        if kontainer_engine::is_valid_network_id(&response.id) {
            Ok(response.id)
        } else {
            Err(AgentClientError::InvalidResponse)
        }
    }

    /// Remove a local network from this node; endpoints come back as [`NetworkRemoval::InUse`].
    pub async fn remove_network(
        &self,
        id: &str,
        cancellation: &CancellationToken,
    ) -> Result<NetworkRemoval, AgentClientError> {
        if !kontainer_engine::is_valid_network_id(id) {
            return Err(AgentClientError::InvalidResponse);
        }
        let deadline = self.rpc_timeout + ACTION_ALLOWANCE;
        let channel = self.channel_with_timeout(cancellation, deadline).await?;
        let mut client = NodeAgentClient::new(channel);
        let mut request = tonic::Request::new(RemoveNetworkRequest { id: id.to_owned() });
        request.set_timeout(deadline);
        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(deadline, client.remove_network(request)) => match result {
                Ok(Ok(response)) => response.into_inner(),
                Ok(Err(status)) => return Err(map_action_status(&status)),
                Err(_) => return Err(AgentClientError::TimedOut),
            }
        };
        if response.in_use_by.len() > MAX_NETWORK_ENDPOINTS
            || response.in_use_by.iter().any(|name| name.len() > 512)
        {
            return Err(AgentClientError::InvalidResponse);
        }
        Ok(if response.in_use_by.is_empty() {
            NetworkRemoval::Removed
        } else {
            NetworkRemoval::InUse(response.in_use_by)
        })
    }

    /// Expected node ID, for shared-store diagnostics without exposing TLS.
    #[must_use]
    pub fn node_id(&self) -> &str {
        self.expected.node_id().unwrap_or_default()
    }
}

/// Controller-owned `Describe` result, distinct from protobuf types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDescription {
    pub node_id: String,
    pub agent_version: String,
    pub architecture: String,
    pub docker_connected: bool,
}

/// Controller-owned container state, distinct from protobuf and Engine types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentContainerState {
    Unknown,
    Created,
    Running,
    Paused,
    Restarting,
    Removing,
    Exited,
    Dead,
    Stopping,
}

/// Controller-owned container summary, distinct from protobuf types. Only the
/// allowlisted fields cross the RPC boundary: no environment, command, mounts,
/// or labels beyond the three Swarm ones.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentContainerSummary {
    pub container_id: String,
    pub display_name: String,
    pub image_reference: String,
    pub state: AgentContainerState,
    pub status: String,
    pub created_at_seconds: i64,
    pub node_id: String,
    pub started_at_seconds: Option<i64>,
    pub finished_at_seconds: Option<i64>,
    pub published_ports: Vec<String>,
    pub service_name: Option<String>,
    pub stack_namespace: Option<String>,
    pub task_id: Option<String>,
    pub cpu_percent: Option<f64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_limit_bytes: Option<u64>,
    /// Networks it has endpoints on, by name.
    pub networks: Vec<String>,
}

/// One image on one node, controller-owned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentImage {
    pub image_id: String,
    pub repo_tags: Vec<String>,
    pub repo_digests: Vec<String>,
    pub size_bytes: u64,
    pub created_at_seconds: i64,
    pub used_by: u32,
    pub dangling: bool,
}

/// One pull event from a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentPullEvent {
    Progress {
        layer_id: Option<String>,
        status: String,
        current: Option<u64>,
        total: Option<u64>,
    },
    /// The registry's or daemon's message, verbatim (bounded by the agent).
    Failed(String),
}

pub type AgentPullStream =
    std::pin::Pin<Box<dyn futures_util::Stream<Item = AgentPullEvent> + Send>>;

/// A shell to open on the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentExecOpen {
    pub container_id: String,
    pub shell: Option<String>,
    pub cols: u32,
    pub rows: u32,
}

/// What the controller relays to the agent after the open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentExecInput {
    Input(Vec<u8>),
    Resize { cols: u32, rows: u32 },
    Close,
}

/// What the agent reports, controller-owned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentExecEvent {
    Started {
        exec_id: String,
        shell: String,
    },
    Output(Vec<u8>),
    /// `None` when the agent doesn't know the exit status. `still_running` when the session
    /// ended but the process didn't: the agent left it running.
    Exit {
        code: Option<i64>,
        still_running: bool,
    },
    Error {
        code: String,
        message: String,
    },
}

/// Ends after `Exit` or `Error`, or when the agent connection drops.
pub type AgentExecStream =
    std::pin::Pin<Box<dyn futures_util::Stream<Item = AgentExecEvent> + Send>>;

const MAX_EXEC_TEXT: usize = 512;

fn bounded(mut text: String, limit: usize) -> String {
    if text.len() > limit {
        let mut end = limit;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}

fn exec_event(frame: exec_server_frame::Frame) -> AgentExecEvent {
    match frame {
        exec_server_frame::Frame::Started(started) => AgentExecEvent::Started {
            exec_id: bounded(started.exec_id, 128),
            shell: bounded(started.shell, 256),
        },
        exec_server_frame::Frame::Output(bytes) => AgentExecEvent::Output(bytes),
        exec_server_frame::Frame::Exit(exit) => AgentExecEvent::Exit {
            code: exit.code,
            still_running: exit.still_running,
        },
        exec_server_frame::Frame::Error(error) => AgentExecEvent::Error {
            code: bounded(error.code, 64),
            message: bounded(error.message, MAX_EXEC_TEXT),
        },
    }
}

fn exec_frame(input: AgentExecInput) -> ExecClientFrame {
    ExecClientFrame {
        frame: Some(match input {
            AgentExecInput::Input(bytes) => exec_client_frame::Frame::Input(bytes),
            AgentExecInput::Resize { cols, rows } => {
                exec_client_frame::Frame::Resize(ExecResizeRequest { cols, rows })
            }
            AgentExecInput::Close => exec_client_frame::Frame::Close(ExecCloseRequest {}),
        }),
    }
}

/// One network as one node sees it, controller-owned.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentNetwork {
    pub id: String,
    pub name: String,
    pub driver: String,
    /// `local`, `swarm`, or `global`.
    pub scope: String,
    pub subnets: Vec<String>,
    /// Running containers on this node with an endpoint on it.
    pub containers: u32,
    pub created_at_seconds: Option<i64>,
    pub labels: std::collections::BTreeMap<String, String>,
    pub attachable: bool,
    pub ingress: bool,
    pub internal: bool,
    pub builtin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentNetworkEndpoint {
    pub container_id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentNetworkDetail {
    pub network: AgentNetwork,
    pub endpoints: Vec<AgentNetworkEndpoint>,
}

/// A local bridge a create asks an agent for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentNetworkCreate {
    pub name: String,
    pub subnet: Option<String>,
    pub labels: std::collections::BTreeMap<String, String>,
}

/// How a local network removal ended when the agent answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkRemoval {
    Removed,
    /// Containers have endpoints on it: nothing was removed.
    InUse(Vec<String>),
}

fn agent_network(network: ProtoNetworkSummary) -> Result<AgentNetwork, AgentClientError> {
    let bounded = |value: &str| value.len() <= 512;
    if !kontainer_engine::is_valid_network_id(&network.id)
        || network.name.is_empty()
        || !bounded(&network.name)
        || !bounded(&network.driver)
        || !matches!(network.scope.as_str(), "local" | "swarm" | "global")
        || network.subnets.len() > 16
        || !network.subnets.iter().all(|subnet| subnet.len() <= 64)
        || network.labels.len() > 64
        || !network
            .labels
            .iter()
            .all(|(key, value)| bounded(key) && bounded(value))
    {
        return Err(AgentClientError::InvalidResponse);
    }
    Ok(AgentNetwork {
        id: network.id,
        name: network.name,
        driver: network.driver,
        scope: network.scope,
        subnets: network.subnets,
        containers: network.containers,
        created_at_seconds: network
            .created_at_unix_seconds
            .filter(|seconds| *seconds >= 0),
        labels: network.labels.into_iter().collect(),
        attachable: network.attachable,
        ingress: network.ingress,
        internal: network.internal,
        builtin: network.builtin,
    })
}

/// One volume on one node, controller-owned.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentVolume {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created_at_seconds: Option<i64>,
    pub labels: std::collections::BTreeMap<String, String>,
    pub used_by: u32,
    pub users: Vec<String>,
    pub size_bytes: Option<u64>,
}

/// What a create asks an agent for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentVolumeCreate {
    pub name: String,
    pub driver: String,
    pub labels: std::collections::BTreeMap<String, String>,
}

/// How a removal ended when the agent answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VolumeRemoval {
    Removed,
    /// Docker refused: these containers mount it. Nothing was removed.
    InUse(Vec<String>),
}

fn agent_volume(volume: ProtoVolumeSummary) -> Result<AgentVolume, AgentClientError> {
    let bounded = |value: &str| value.len() <= 512;
    if volume.name.is_empty()
        || !bounded(&volume.name)
        || !bounded(&volume.driver)
        || volume.mountpoint.len() > 4096
        || volume.labels.len() > 64
        || !volume
            .labels
            .iter()
            .all(|(key, value)| bounded(key) && bounded(value))
        || volume.users.len() > MAX_VOLUME_USERS
        || !volume.users.iter().all(|user| bounded(user))
    {
        return Err(AgentClientError::InvalidResponse);
    }
    Ok(AgentVolume {
        name: volume.name,
        driver: volume.driver,
        mountpoint: volume.mountpoint,
        created_at_seconds: volume
            .created_at_unix_seconds
            .filter(|seconds| *seconds >= 0),
        labels: volume.labels.into_iter().collect(),
        used_by: volume.used_by,
        users: volume.users,
        size_bytes: volume.size_bytes,
    })
}

/// Lifecycle actions the controller asks an agent for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentContainerAction {
    Start,
    Stop,
    Restart,
    Pause,
    Unpause,
    Remove,
}

impl AgentContainerAction {
    pub const ALL: [Self; 6] = [
        Self::Start,
        Self::Stop,
        Self::Restart,
        Self::Pause,
        Self::Unpause,
        Self::Remove,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Pause => "pause",
            Self::Unpause => "unpause",
            Self::Remove => "remove",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|action| action.as_str() == value)
    }

    const fn to_proto(self) -> ProtoContainerAction {
        match self {
            Self::Start => ProtoContainerAction::Start,
            Self::Stop => ProtoContainerAction::Stop,
            Self::Restart => ProtoContainerAction::Restart,
            Self::Pause => ProtoContainerAction::Pause,
            Self::Unpause => ProtoContainerAction::Unpause,
            Self::Remove => ProtoContainerAction::Remove,
        }
    }
}

/// Controller-owned container page preserving agent order and cursor.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentContainerPage {
    pub containers: Vec<AgentContainerSummary>,
    pub next_page_token: Option<String>,
}

/// Narrow injectable handle for node diagnostics and inventory reads.
///
/// Production inserts the configured [`NodeAgentClientConfig`]; tests insert
/// a fake. No request input can select an endpoint, DNS name, or identity.
#[async_trait::async_trait]
pub trait AgentClient: std::fmt::Debug + Send + Sync {
    async fn describe(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<AgentDescription, AgentClientError>;

    async fn list_containers(
        &self,
        page_size: u32,
        page_token: String,
        cancellation: &CancellationToken,
    ) -> Result<AgentContainerPage, AgentClientError>;

    async fn container_action(
        &self,
        _container_id: &str,
        _action: AgentContainerAction,
        _force: bool,
        _timeout_seconds: Option<u32>,
        _cancellation: &CancellationToken,
    ) -> Result<(), AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    async fn list_images(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<AgentImage>, AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    async fn remove_image(
        &self,
        _image_id: &str,
        _force: bool,
        _cancellation: &CancellationToken,
    ) -> Result<(), AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    async fn pull_image(
        &self,
        _reference: &str,
        _cancellation: &CancellationToken,
    ) -> Result<AgentPullStream, AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    async fn exec(
        &self,
        _open: AgentExecOpen,
        _input: tokio::sync::mpsc::Receiver<AgentExecInput>,
        _cancellation: &CancellationToken,
    ) -> Result<AgentExecStream, AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    async fn list_volumes(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<AgentVolume>, AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    async fn list_networks(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<AgentNetwork>, AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    async fn inspect_network(
        &self,
        _id: &str,
        _cancellation: &CancellationToken,
    ) -> Result<AgentNetworkDetail, AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    async fn create_network(
        &self,
        _network: &AgentNetworkCreate,
        _cancellation: &CancellationToken,
    ) -> Result<String, AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    async fn remove_network(
        &self,
        _id: &str,
        _cancellation: &CancellationToken,
    ) -> Result<NetworkRemoval, AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    async fn create_volume(
        &self,
        _volume: &AgentVolumeCreate,
        _cancellation: &CancellationToken,
    ) -> Result<AgentVolume, AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    async fn remove_volume(
        &self,
        _name: &str,
        _cancellation: &CancellationToken,
    ) -> Result<VolumeRemoval, AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    fn node_id(&self) -> &str;
}

#[async_trait::async_trait]
impl AgentClient for NodeAgentClientConfig {
    async fn exec(
        &self,
        open: AgentExecOpen,
        input: tokio::sync::mpsc::Receiver<AgentExecInput>,
        cancellation: &CancellationToken,
    ) -> Result<AgentExecStream, AgentClientError> {
        NodeAgentClientConfig::exec(self, open, input, cancellation).await
    }

    async fn describe(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<AgentDescription, AgentClientError> {
        NodeAgentClientConfig::describe(self, cancellation).await
    }

    async fn list_containers(
        &self,
        page_size: u32,
        page_token: String,
        cancellation: &CancellationToken,
    ) -> Result<AgentContainerPage, AgentClientError> {
        NodeAgentClientConfig::list_containers(self, page_size, page_token, cancellation).await
    }

    async fn container_action(
        &self,
        container_id: &str,
        action: AgentContainerAction,
        force: bool,
        timeout_seconds: Option<u32>,
        cancellation: &CancellationToken,
    ) -> Result<(), AgentClientError> {
        NodeAgentClientConfig::container_action(
            self,
            container_id,
            action,
            force,
            timeout_seconds,
            cancellation,
        )
        .await
    }

    async fn list_images(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<AgentImage>, AgentClientError> {
        NodeAgentClientConfig::list_images(self, cancellation).await
    }

    async fn remove_image(
        &self,
        image_id: &str,
        force: bool,
        cancellation: &CancellationToken,
    ) -> Result<(), AgentClientError> {
        NodeAgentClientConfig::remove_image(self, image_id, force, cancellation).await
    }

    async fn pull_image(
        &self,
        reference: &str,
        cancellation: &CancellationToken,
    ) -> Result<AgentPullStream, AgentClientError> {
        NodeAgentClientConfig::pull_image(self, reference, cancellation).await
    }

    async fn list_volumes(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<AgentVolume>, AgentClientError> {
        NodeAgentClientConfig::list_volumes(self, cancellation).await
    }

    async fn list_networks(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<AgentNetwork>, AgentClientError> {
        NodeAgentClientConfig::list_networks(self, cancellation).await
    }

    async fn inspect_network(
        &self,
        id: &str,
        cancellation: &CancellationToken,
    ) -> Result<AgentNetworkDetail, AgentClientError> {
        NodeAgentClientConfig::inspect_network(self, id, cancellation).await
    }

    async fn create_network(
        &self,
        network: &AgentNetworkCreate,
        cancellation: &CancellationToken,
    ) -> Result<String, AgentClientError> {
        NodeAgentClientConfig::create_network(self, network, cancellation).await
    }

    async fn remove_network(
        &self,
        id: &str,
        cancellation: &CancellationToken,
    ) -> Result<NetworkRemoval, AgentClientError> {
        NodeAgentClientConfig::remove_network(self, id, cancellation).await
    }

    async fn create_volume(
        &self,
        volume: &AgentVolumeCreate,
        cancellation: &CancellationToken,
    ) -> Result<AgentVolume, AgentClientError> {
        NodeAgentClientConfig::create_volume(self, volume, cancellation).await
    }

    async fn remove_volume(
        &self,
        name: &str,
        cancellation: &CancellationToken,
    ) -> Result<VolumeRemoval, AgentClientError> {
        NodeAgentClientConfig::remove_volume(self, name, cancellation).await
    }

    fn node_id(&self) -> &str {
        NodeAgentClientConfig::node_id(self)
    }
}

/// Stable, redacted call failures. No endpoint, address, TLS, or daemon
/// detail crosses this boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentClientError {
    /// The agent could not be reached or its certificate was rejected.
    Unavailable,
    /// The connect or RPC deadline expired.
    TimedOut,
    /// The caller or controller shutdown cancelled the call.
    Cancelled,
    /// The peer identity or reported node ID did not match configuration.
    IdentityMismatch,
    /// The response was malformed.
    InvalidResponse,
    /// Pagination was rejected locally or by the agent (including a stale
    /// cursor whose live target disappeared).
    InvalidPagination,
    /// The container does not exist on that node.
    NotFound,
    /// The container is already in the requested state.
    AlreadyInState,
    /// Removing a running container needs `force`.
    ContainerRunning,
    /// The node's Docker Engine refused or failed.
    EngineError,
    /// Containers use the image; removing it needs `force`.
    ImageInUse,
    /// A volume with that name already exists on the node.
    VolumeExists,
    /// Docker's own networks can't be removed.
    BuiltinNetwork,
}

impl std::fmt::Display for AgentClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("agent is unavailable"),
            Self::TimedOut => formatter.write_str("agent request timed out"),
            Self::Cancelled => formatter.write_str("agent request was cancelled"),
            Self::IdentityMismatch => formatter.write_str("agent identity did not match"),
            Self::InvalidResponse => formatter.write_str("agent response was invalid"),
            Self::InvalidPagination => formatter.write_str("pagination is invalid"),
            Self::NotFound => formatter.write_str("container not found"),
            Self::AlreadyInState => formatter.write_str("container is already in that state"),
            Self::ContainerRunning => formatter.write_str("container is running"),
            Self::EngineError => formatter.write_str("engine failed the action"),
            Self::ImageInUse => formatter.write_str("image is in use"),
            Self::VolumeExists => formatter.write_str("volume already exists"),
            Self::BuiltinNetwork => formatter.write_str("built-in network"),
        }
    }
}

impl std::error::Error for AgentClientError {}

/// Startup validation failures. Messages stay generic and never include
/// certificate material or secret values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentClientConfigError {
    InvalidEndpoint,
    InvalidServerName,
    InvalidIdentity(&'static str),
    InvalidTls(String),
}

impl std::fmt::Display for AgentClientConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidEndpoint => formatter.write_str("agent endpoint is invalid"),
            Self::InvalidServerName => formatter.write_str("agent server name is invalid"),
            Self::InvalidIdentity(which) => {
                write!(formatter, "agent {which} is invalid")
            }
            Self::InvalidTls(detail) => formatter.write_str(detail),
        }
    }
}

impl std::error::Error for AgentClientConfigError {}

fn validate_endpoint(endpoint: &str) -> Result<(), AgentClientConfigError> {
    let parsed = url::Url::parse(endpoint).map_err(|_| AgentClientConfigError::InvalidEndpoint)?;
    if parsed.scheme() != "https" {
        return Err(AgentClientConfigError::InvalidEndpoint);
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AgentClientConfigError::InvalidEndpoint);
    }
    if parsed.host_str().is_none() {
        return Err(AgentClientConfigError::InvalidEndpoint);
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(AgentClientConfigError::InvalidEndpoint);
    }
    // A routing path would allow one URL to address different agents;
    // the static endpoint must name exactly one agent origin.
    if parsed.path() != "/" && !parsed.path().is_empty() {
        return Err(AgentClientConfigError::InvalidEndpoint);
    }
    Ok(())
}

fn validate_server_name(server_name: &str) -> Result<(), AgentClientConfigError> {
    if server_name.is_empty() || server_name.len() > 253 {
        return Err(AgentClientConfigError::InvalidServerName);
    }
    if !server_name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
    {
        return Err(AgentClientConfigError::InvalidServerName);
    }
    if server_name.starts_with('-')
        || server_name.starts_with('.')
        || server_name.ends_with('-')
        || server_name.ends_with('.')
        || server_name.contains("..")
    {
        return Err(AgentClientConfigError::InvalidServerName);
    }
    Ok(())
}

fn validate_identity_segment(
    value: &str,
    which: &'static str,
) -> Result<(), AgentClientConfigError> {
    if !kontainer_rpc_auth::valid_identity_segment(value) {
        return Err(AgentClientConfigError::InvalidIdentity(which));
    }
    Ok(())
}

fn map_rpc_status(status: tonic::Status) -> AgentClientError {
    use tonic::Code;
    match status.code() {
        Code::Cancelled => AgentClientError::Cancelled,
        Code::DeadlineExceeded => AgentClientError::TimedOut,
        Code::Unauthenticated | Code::PermissionDenied => AgentClientError::IdentityMismatch,
        Code::Unavailable => AgentClientError::Unavailable,
        Code::InvalidArgument => AgentClientError::InvalidPagination,
        _ => AgentClientError::InvalidResponse,
    }
}

/// Status codes from `ContainerAction`, read from the stable message the agent sets.
fn map_action_status(status: &tonic::Status) -> AgentClientError {
    use tonic::Code;
    match (status.code(), status.message()) {
        (Code::NotFound, _) => AgentClientError::NotFound,
        (Code::FailedPrecondition, "already_in_state") => AgentClientError::AlreadyInState,
        (Code::FailedPrecondition, "container_running") => AgentClientError::ContainerRunning,
        (Code::FailedPrecondition, "image_in_use") => AgentClientError::ImageInUse,
        (Code::AlreadyExists, "volume_exists") => AgentClientError::VolumeExists,
        (Code::InvalidArgument, "builtin_network") => AgentClientError::BuiltinNetwork,
        (Code::DeadlineExceeded, _) => AgentClientError::TimedOut,
        (Code::Cancelled, _) => AgentClientError::Cancelled,
        (Code::Unauthenticated | Code::PermissionDenied, _) => AgentClientError::IdentityMismatch,
        (Code::Unavailable, "engine_error") => AgentClientError::EngineError,
        (Code::Unavailable, _) => AgentClientError::Unavailable,
        _ => AgentClientError::InvalidResponse,
    }
}

/// Extra time an action gets beyond the RPC timeout and the stop grace.
const ACTION_ALLOWANCE: Duration = Duration::from_secs(20);
/// A pull stream may run as long as the `image.pull` operation.
const PULL_DEADLINE: Duration = Duration::from_secs(60 * 60);
/// An exec stream lives as long as the session may (8 h), plus a minute.
const EXEC_DEADLINE: Duration = Duration::from_secs(8 * 60 * 60 + 60);
const MAX_IMAGES_PER_NODE: usize = 2_000;
const MAX_VOLUMES_PER_NODE: usize = 2_000;
const MAX_CONTAINER_NETWORKS: usize = 16;
const MAX_NETWORKS_PER_NODE: usize = 1_000;
const MAX_NETWORK_ENDPOINTS: usize = 256;
const MAX_VOLUME_USERS: usize = 16;

fn is_valid_image_id(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(is_valid_container_id)
}

/// #82 pagination bounds mirrored without an Engine dependency so the
/// controller never forwards an unbounded page or cursor shape.
pub(crate) const MAX_CONTAINER_PAGE_SIZE: usize = 500;
const MAX_PUBLISHED_PORTS: usize = 64;
const MAX_LABEL_LEN: usize = 255;
pub(crate) const MAX_CONTAINER_PAGE_TOKEN_LEN: usize = 67;
const MAX_CONTAINER_STATUS_LEN: usize = 256;
const CONTAINER_ID_LEN: usize = 64;

pub(crate) fn is_valid_container_id(value: &str) -> bool {
    value.len() == CONTAINER_ID_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn is_valid_page_token(token: &str) -> bool {
    if token.is_empty() || token.len() > MAX_CONTAINER_PAGE_TOKEN_LEN {
        return false;
    }
    token.strip_prefix("v1:").is_some_and(is_valid_container_id)
}

/// Validate one proto summary against the configured node and the #82
/// required fields. Mirrors the Engine adapter so a negative timestamp,
/// missing display name, or blank image never becomes a 200 page.
fn validate_summary(
    item: &kontainer_agent_proto::v1::ContainerSummary,
    expected_node: &str,
) -> Result<AgentContainerState, AgentClientError> {
    if !is_valid_container_id(&item.container_id) {
        return Err(AgentClientError::InvalidResponse);
    }
    if item.node_id != expected_node {
        return Err(AgentClientError::InvalidResponse);
    }
    if item.status.len() > MAX_CONTAINER_STATUS_LEN {
        return Err(AgentClientError::InvalidResponse);
    }
    if item.created_at_unix_seconds < 0
        || item.display_name.is_empty()
        || item.image_reference.trim().is_empty()
    {
        return Err(AgentClientError::InvalidResponse);
    }
    let port_ok = |port: &String| {
        port.len() <= 24
            && port
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b':' || byte == b'/')
    };
    let label_ok = |label: &Option<String>| {
        label
            .as_ref()
            .is_none_or(|label| !label.is_empty() && label.len() <= MAX_LABEL_LEN)
    };
    let figure_ok =
        |value: Option<f64>| value.is_none_or(|value| value.is_finite() && value >= 0.0);
    if item.published_ports.len() > MAX_PUBLISHED_PORTS
        || !item.published_ports.iter().all(port_ok)
        || !label_ok(&item.service_name)
        || !label_ok(&item.stack_namespace)
        || !label_ok(&item.task_id)
        || !figure_ok(item.cpu_percent)
        || item.networks.len() > MAX_CONTAINER_NETWORKS
        || !item
            .networks
            .iter()
            .all(|network| kontainer_engine::is_valid_network_name(network))
    {
        return Err(AgentClientError::InvalidResponse);
    }
    match ProtoContainerState::try_from(item.state) {
        Ok(ProtoContainerState::Unknown) => Ok(AgentContainerState::Unknown),
        Ok(ProtoContainerState::Created) => Ok(AgentContainerState::Created),
        Ok(ProtoContainerState::Running) => Ok(AgentContainerState::Running),
        Ok(ProtoContainerState::Paused) => Ok(AgentContainerState::Paused),
        Ok(ProtoContainerState::Restarting) => Ok(AgentContainerState::Restarting),
        Ok(ProtoContainerState::Removing) => Ok(AgentContainerState::Removing),
        Ok(ProtoContainerState::Exited) => Ok(AgentContainerState::Exited),
        Ok(ProtoContainerState::Dead) => Ok(AgentContainerState::Dead),
        Ok(ProtoContainerState::Stopping) => Ok(AgentContainerState::Stopping),
        Ok(ProtoContainerState::Unspecified) | Err(_) => Err(AgentClientError::InvalidResponse),
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentClientConfigError, validate_endpoint, validate_server_name};

    #[test]
    fn endpoint_rejects_non_static_origins() {
        for endpoint in [
            "",
            "http://127.0.0.1:50052",
            "https://127.0.0.1:50052/containers",
            "https://127.0.0.1:50052/?cursor=1",
            "https://127.0.0.1:50052#fragment",
            "https://user:pass@127.0.0.1:50052/",
            "https://[::1",
            "not-a-url",
        ] {
            assert_eq!(
                validate_endpoint(endpoint),
                Err(AgentClientConfigError::InvalidEndpoint),
                "{endpoint}"
            );
        }
        assert!(validate_endpoint("https://127.0.0.1:50052").is_ok());
        assert!(validate_endpoint("https://127.0.0.1:50052/").is_ok());
    }

    #[test]
    fn server_names_reject_empty_and_malformed_hosts() {
        for name in [
            "",
            "-agent.test",
            ".agent.test",
            "agent..test",
            "agent_test",
            "agent test",
        ] {
            assert_eq!(
                validate_server_name(name),
                Err(AgentClientConfigError::InvalidServerName),
                "{name}"
            );
        }
        assert!(validate_server_name("node-1.agent.kontainer.test").is_ok());
    }

    #[test]
    fn error_messages_stay_generic() {
        assert_eq!(
            AgentClientConfigError::InvalidEndpoint.to_string(),
            "agent endpoint is invalid"
        );
        assert_eq!(
            super::AgentClientError::Unavailable.to_string(),
            "agent is unavailable"
        );
        assert_eq!(
            super::AgentClientError::IdentityMismatch.to_string(),
            "agent identity did not match"
        );
        assert_eq!(
            super::AgentClientError::InvalidPagination.to_string(),
            "pagination is invalid"
        );
        assert_eq!(
            super::map_rpc_status(tonic::Status::invalid_argument("stale cursor")),
            super::AgentClientError::InvalidPagination
        );
        assert_eq!(
            super::map_rpc_status(tonic::Status::deadline_exceeded("slow")),
            super::AgentClientError::TimedOut
        );
        assert_eq!(
            super::map_rpc_status(tonic::Status::unavailable("down")),
            super::AgentClientError::Unavailable
        );
    }

    #[test]
    fn container_ids_and_page_tokens_use_the_agent_bounds() {
        let id = "a".repeat(64);
        assert!(super::is_valid_container_id(&id));
        assert!(!super::is_valid_container_id("abc"));
        assert!(!super::is_valid_container_id(&"A".repeat(64)));
        assert!(super::is_valid_page_token(&format!("v1:{id}")));
        assert!(!super::is_valid_page_token(""));
        assert!(!super::is_valid_page_token("not-a-cursor"));
        assert!(!super::is_valid_page_token("v1:short"));
        assert!(!super::is_valid_page_token(&"x".repeat(68)));
    }

    #[test]
    fn summaries_require_engine_mandatory_fields() {
        use kontainer_agent_proto::v1::{
            ContainerState as ProtoState, ContainerSummary as ProtoSummary,
        };
        let valid = ProtoSummary {
            container_id: "a".repeat(64),
            display_name: "web".to_owned(),
            image_reference: "nginx:latest".to_owned(),
            state: ProtoState::Running as i32,
            status: "Up 3s".to_owned(),
            created_at_unix_seconds: 1_700_000_000,
            node_id: "node-1".to_owned(),
            ..ProtoSummary::default()
        };
        assert!(super::validate_summary(&valid, "node-1").is_ok());
        for mutate in [
            ("negative time", {
                let mut item = valid.clone();
                item.created_at_unix_seconds = -1;
                item
            }),
            ("empty display", {
                let mut item = valid.clone();
                item.display_name.clear();
                item
            }),
            ("blank image", {
                let mut item = valid.clone();
                item.image_reference = "   ".to_owned();
                item
            }),
            ("wrong node", {
                let mut item = valid.clone();
                item.node_id = "node-2".to_owned();
                item
            }),
            ("unspecified state", {
                let mut item = valid.clone();
                item.state = ProtoState::Unspecified as i32;
                item
            }),
        ] {
            assert_eq!(
                super::validate_summary(&mutate.1, "node-1"),
                Err(super::AgentClientError::InvalidResponse),
                "{} must be an invalid response",
                mutate.0
            );
        }
    }
}
