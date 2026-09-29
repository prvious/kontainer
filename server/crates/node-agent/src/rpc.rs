use std::sync::Arc;

use kontainer_agent_proto::v1::ExecClientFrame;
use kontainer_agent_proto::v1::{
    ContainerAction as ProtoContainerAction, ContainerActionRequest, ContainerActionResponse,
    ContainerState as ProtoContainerState, ContainerSummary as ProtoContainerSummary,
    DescribeRequest, DescribeResponse, ImageSummary as ProtoImageSummary, ListContainersRequest,
    ListContainersResponse, ListImagesRequest, ListImagesResponse, PullImageEvent,
    PullImageRequest, RemoveImageRequest, RemoveImageResponse, node_agent_server::NodeAgent,
};
use kontainer_agent_proto::v1::{
    CreateNetworkRequest, CreateNetworkResponse, InspectNetworkRequest, InspectNetworkResponse,
    ListNetworksRequest, ListNetworksResponse, NetworkEndpoint as ProtoNetworkEndpoint,
    NetworkSummary as ProtoNetworkSummary, RemoveNetworkRequest, RemoveNetworkResponse,
};
use kontainer_agent_proto::v1::{
    CreateVolumeRequest, CreateVolumeResponse, ListVolumesRequest, ListVolumesResponse,
    RemoveVolumeRequest, RemoveVolumeResponse, VolumeSummary as ProtoVolumeSummary,
};
use kontainer_engine::{
    ContainerAction, ContainerActionOptions, ContainerEngine, ContainerPageRequest, ContainerState,
    EngineError, EngineErrorKind, MAX_STOP_TIMEOUT_SECONDS, PullEvent, is_plausible_reference,
    is_valid_container_id, is_valid_image_id,
};
use kontainer_engine::{
    NetworkCreate, NetworkSummary, VolumeCreate, VolumeSummary, are_valid_volume_labels,
    is_builtin_network, is_valid_cidr, is_valid_network_id, is_valid_network_name,
    is_valid_volume_name,
};
use kontainer_rpc_auth::AuthenticatedPeerIdentity;
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status};
use tracing::warn;

use crate::{
    container_times::ContainerTimesCache,
    sampling::{ContainerStatsCache, VolumeSizeCache},
};

#[derive(Clone)]
pub struct NodeAgentService {
    node_id: String,
    engine: Arc<dyn ContainerEngine>,
    cancellation: CancellationToken,
    stats: ContainerStatsCache,
    times: ContainerTimesCache,
    volume_sizes: VolumeSizeCache,
}

impl NodeAgentService {
    #[must_use]
    pub fn new(
        node_id: impl Into<String>,
        engine: Arc<dyn ContainerEngine>,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            engine,
            cancellation,
            stats: ContainerStatsCache::default(),
            times: ContainerTimesCache::default(),
            volume_sizes: VolumeSizeCache::default(),
        }
    }

    /// Serve container CPU and memory from the sampler's last tick.
    #[must_use]
    pub fn with_stats(mut self, stats: ContainerStatsCache) -> Self {
        self.stats = stats;
        self
    }

    /// Serve volume sizes from the sampler's last `docker system df`.
    #[must_use]
    pub fn with_volume_sizes(mut self, volume_sizes: VolumeSizeCache) -> Self {
        self.volume_sizes = volume_sizes;
        self
    }

    fn volume_to_proto(&self, volume: VolumeSummary) -> ProtoVolumeSummary {
        ProtoVolumeSummary {
            size_bytes: volume
                .size_bytes
                .or_else(|| self.volume_sizes.get(&volume.name)),
            name: volume.name,
            driver: volume.driver,
            mountpoint: volume.mountpoint,
            created_at_unix_seconds: volume.created_at_seconds,
            labels: volume.labels.into_iter().collect(),
            used_by: volume.used_by,
            users: volume.users,
        }
    }
}

fn require_controller<T>(request: &Request<T>) -> Result<(), Status> {
    request
        .extensions()
        .get::<AuthenticatedPeerIdentity>()
        .map(|_| ())
        .ok_or_else(|| Status::unauthenticated("authenticated controller identity is required"))
}

pub type PullImageStream =
    std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<PullImageEvent, Status>> + Send>>;

#[tonic::async_trait]
impl NodeAgent for NodeAgentService {
    type PullImageStream = PullImageStream;
    type ExecStream = crate::exec::ExecStream;

    async fn exec(
        &self,
        request: Request<tonic::Streaming<ExecClientFrame>>,
    ) -> Result<Response<Self::ExecStream>, Status> {
        require_controller(&request)?;
        crate::exec::open(
            self.engine.clone(),
            request.into_inner(),
            self.cancellation.child_token(),
        )
        .await
        .map(Response::new)
    }

    async fn list_images(
        &self,
        request: Request<ListImagesRequest>,
    ) -> Result<Response<ListImagesResponse>, Status> {
        require_controller(&request)?;
        let images = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => {
                return Err(Status::cancelled("node agent is shutting down"));
            }
            images = self.engine.list_images(&self.cancellation) => images.map_err(|error| {
                warn!(%error, "image listing failed");
                engine_error_to_status(&error)
            })?,
        };
        Ok(Response::new(ListImagesResponse {
            images: images
                .into_iter()
                .map(|image| ProtoImageSummary {
                    image_id: image.image_id,
                    repo_tags: image.repo_tags,
                    repo_digests: image.repo_digests,
                    size_bytes: image.size_bytes,
                    created_at_unix_seconds: image.created_at_seconds,
                    used_by: image.used_by,
                    dangling: image.dangling,
                })
                .collect(),
        }))
    }

    async fn remove_image(
        &self,
        request: Request<RemoveImageRequest>,
    ) -> Result<Response<RemoveImageResponse>, Status> {
        require_controller(&request)?;
        let inner = request.into_inner();
        if !is_valid_image_id(&inner.image_id) {
            return Err(Status::invalid_argument("invalid_image"));
        }
        let result = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => {
                return Err(Status::cancelled("node agent is shutting down"));
            }
            result = self.engine.remove_image(&inner.image_id, inner.force, &self.cancellation) => result,
        };
        result.map_err(|error| {
            warn!(%error, "image removal failed");
            match error.kind() {
                EngineErrorKind::Conflict => Status::failed_precondition("image_in_use"),
                _ => action_error_to_status(&error),
            }
        })?;
        Ok(Response::new(RemoveImageResponse {}))
    }

    async fn list_networks(
        &self,
        request: Request<ListNetworksRequest>,
    ) -> Result<Response<ListNetworksResponse>, Status> {
        require_controller(&request)?;
        let networks = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => {
                return Err(Status::cancelled("node agent is shutting down"));
            }
            networks = self.engine.list_networks(&self.cancellation) => networks.map_err(|error| {
                warn!(%error, "network listing failed");
                engine_error_to_status(&error)
            })?,
        };
        Ok(Response::new(ListNetworksResponse {
            networks: networks.into_iter().map(network_to_proto).collect(),
        }))
    }

    async fn inspect_network(
        &self,
        request: Request<InspectNetworkRequest>,
    ) -> Result<Response<InspectNetworkResponse>, Status> {
        require_controller(&request)?;
        let id = request.into_inner().id;
        if !is_valid_network_id(&id) {
            return Err(Status::invalid_argument("invalid_network"));
        }
        let detail = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => {
                return Err(Status::cancelled("node agent is shutting down"));
            }
            detail = self.engine.inspect_network(&id, &self.cancellation) => detail.map_err(|error| {
                warn!(%error, "network inspect failed");
                action_error_to_status(&error)
            })?,
        };
        Ok(Response::new(InspectNetworkResponse {
            network: Some(network_to_proto(detail.network)),
            endpoints: detail
                .endpoints
                .into_iter()
                .map(|endpoint| ProtoNetworkEndpoint {
                    container_id: endpoint.container_id,
                    name: endpoint.name,
                })
                .collect(),
        }))
    }

    async fn create_network(
        &self,
        request: Request<CreateNetworkRequest>,
    ) -> Result<Response<CreateNetworkResponse>, Status> {
        require_controller(&request)?;
        let inner = request.into_inner();
        let network = NetworkCreate {
            name: inner.name,
            driver: "bridge".to_owned(),
            attachable: false,
            subnet: inner.subnet,
            labels: inner.labels.into_iter().collect(),
        };
        if !is_valid_network_name(&network.name)
            || is_builtin_network(&network.name)
            || network
                .subnet
                .as_deref()
                .is_some_and(|subnet| !is_valid_cidr(subnet))
            || !are_valid_volume_labels(&network.labels)
        {
            return Err(Status::invalid_argument("invalid_network"));
        }
        let result = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => {
                return Err(Status::cancelled("node agent is shutting down"));
            }
            result = self.engine.create_network(&network, &self.cancellation) => result,
        };
        let id = result.map_err(|error| {
            warn!(%error, "network creation failed");
            action_error_to_status(&error)
        })?;
        Ok(Response::new(CreateNetworkResponse { id }))
    }

    async fn remove_network(
        &self,
        request: Request<RemoveNetworkRequest>,
    ) -> Result<Response<RemoveNetworkResponse>, Status> {
        require_controller(&request)?;
        let id = request.into_inner().id;
        if !is_valid_network_id(&id) {
            return Err(Status::invalid_argument("invalid_network"));
        }
        let detail = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => {
                return Err(Status::cancelled("node agent is shutting down"));
            }
            detail = self.engine.inspect_network(&id, &self.cancellation) => detail.map_err(|error| {
                warn!(%error, "network inspect before removal failed");
                action_error_to_status(&error)
            })?,
        };
        if detail.network.builtin() {
            return Err(Status::invalid_argument("builtin_network"));
        }
        if !detail.endpoints.is_empty() {
            return Ok(Response::new(RemoveNetworkResponse {
                in_use_by: detail
                    .endpoints
                    .into_iter()
                    .map(|endpoint| endpoint.name)
                    .collect(),
            }));
        }
        let result = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => {
                return Err(Status::cancelled("node agent is shutting down"));
            }
            result = self.engine.remove_network(&id, &self.cancellation) => result,
        };
        result.map_err(|error| {
            warn!(%error, "network removal failed");
            action_error_to_status(&error)
        })?;
        Ok(Response::new(RemoveNetworkResponse {
            in_use_by: Vec::new(),
        }))
    }

    async fn list_volumes(
        &self,
        request: Request<ListVolumesRequest>,
    ) -> Result<Response<ListVolumesResponse>, Status> {
        require_controller(&request)?;
        let volumes = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => {
                return Err(Status::cancelled("node agent is shutting down"));
            }
            volumes = self.engine.list_volumes(&self.cancellation) => volumes.map_err(|error| {
                warn!(%error, "volume listing failed");
                engine_error_to_status(&error)
            })?,
        };
        Ok(Response::new(ListVolumesResponse {
            volumes: volumes
                .into_iter()
                .map(|volume| self.volume_to_proto(volume))
                .collect(),
        }))
    }

    async fn create_volume(
        &self,
        request: Request<CreateVolumeRequest>,
    ) -> Result<Response<CreateVolumeResponse>, Status> {
        require_controller(&request)?;
        let inner = request.into_inner();
        let volume = VolumeCreate {
            name: inner.name,
            driver: inner.driver,
            labels: inner.labels.into_iter().collect(),
            driver_opts: inner.driver_opts.into_iter().collect(),
        };
        if !is_valid_volume_name(&volume.name)
            || volume.driver.is_empty()
            || volume.driver.len() > 64
            || !are_valid_volume_labels(&volume.labels)
            || !are_valid_volume_labels(&volume.driver_opts)
        {
            return Err(Status::invalid_argument("invalid_volume"));
        }
        let result = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => {
                return Err(Status::cancelled("node agent is shutting down"));
            }
            result = self.engine.create_volume(&volume, &self.cancellation) => result,
        };
        let created = result.map_err(|error| {
            warn!(%error, "volume creation failed");
            match error.kind() {
                EngineErrorKind::Conflict => Status::already_exists("volume_exists"),
                _ => action_error_to_status(&error),
            }
        })?;
        Ok(Response::new(CreateVolumeResponse {
            volume: Some(self.volume_to_proto(created)),
        }))
    }

    async fn remove_volume(
        &self,
        request: Request<RemoveVolumeRequest>,
    ) -> Result<Response<RemoveVolumeResponse>, Status> {
        require_controller(&request)?;
        let name = request.into_inner().name;
        if !is_valid_volume_name(&name) {
            return Err(Status::invalid_argument("invalid_volume"));
        }
        let result = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => {
                return Err(Status::cancelled("node agent is shutting down"));
            }
            result = self.engine.remove_volume(&name, &self.cancellation) => result,
        };
        match result {
            Ok(()) => Ok(Response::new(RemoveVolumeResponse {
                in_use_by: Vec::new(),
            })),
            // Docker's 409 names container ids; list the volume's users by name instead.
            Err(error) if error.kind() == EngineErrorKind::Conflict => {
                let users = self
                    .engine
                    .list_volumes(&self.cancellation)
                    .await
                    .ok()
                    .and_then(|volumes| volumes.into_iter().find(|volume| volume.name == name))
                    .map(|volume| volume.users)
                    .filter(|users| !users.is_empty())
                    .unwrap_or_else(|| vec!["a container".to_owned()]);
                Ok(Response::new(RemoveVolumeResponse { in_use_by: users }))
            }
            Err(error) => {
                warn!(%error, "volume removal failed");
                Err(action_error_to_status(&error))
            }
        }
    }

    async fn pull_image(
        &self,
        request: Request<PullImageRequest>,
    ) -> Result<Response<Self::PullImageStream>, Status> {
        use futures_util::StreamExt as _;

        require_controller(&request)?;
        let reference = request.into_inner().reference;
        if !is_plausible_reference(&reference) {
            return Err(Status::invalid_argument("invalid_reference"));
        }
        let stream = self
            .engine
            .pull_image(&reference, &self.cancellation)
            .await
            .map_err(|error| {
                warn!(%error, "image pull could not start");
                action_error_to_status(&error)
            })?;
        Ok(Response::new(Box::pin(stream.map(|event| {
            Ok(match event {
                PullEvent::Progress(progress) => PullImageEvent {
                    layer_id: progress.layer_id,
                    status: progress.status,
                    current: progress.current,
                    total: progress.total,
                    error: None,
                },
                PullEvent::Failed(message) => PullImageEvent {
                    layer_id: None,
                    status: "failed".to_owned(),
                    current: None,
                    total: None,
                    error: Some(message),
                },
            })
        }))))
    }

    async fn describe(
        &self,
        request: Request<DescribeRequest>,
    ) -> Result<Response<DescribeResponse>, Status> {
        request
            .extensions()
            .get::<AuthenticatedPeerIdentity>()
            .ok_or_else(|| {
                Status::unauthenticated("authenticated controller identity is required")
            })?;

        let docker_connected = tokio::select! {
            _ = self.cancellation.cancelled() => {
                return Err(Status::cancelled("node agent is shutting down"));
            }
            availability = self.engine.is_available() => match availability {
                Ok(is_available) => is_available,
                Err(error) => {
                    warn!(%error, "Docker health check failed");
                    false
                }
            }
        };

        Ok(Response::new(DescribeResponse {
            node_id: self.node_id.clone(),
            agent_version: env!("CARGO_PKG_VERSION").to_owned(),
            architecture: std::env::consts::ARCH.to_owned(),
            docker_connected,
        }))
    }

    async fn list_containers(
        &self,
        request: Request<ListContainersRequest>,
    ) -> Result<Response<ListContainersResponse>, Status> {
        request
            .extensions()
            .get::<AuthenticatedPeerIdentity>()
            .ok_or_else(|| {
                Status::unauthenticated("authenticated controller identity is required")
            })?;

        let inner = request.into_inner();
        let page_request = ContainerPageRequest::new(inner.page_size, &inner.page_token)
            .map_err(|_| Status::invalid_argument("pagination is invalid"))?;

        let page = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => {
                return Err(Status::cancelled("node agent is shutting down"));
            }
            page = self.engine.list_containers(page_request, &self.cancellation) => {
                page.map_err(|error| {
                    // Stable kinds only; daemon detail stays in the error value,
                    // never in the message.
                    warn!(%error, "container inventory failed");
                    engine_error_to_status(&error)
                })?
            }
        };

        let times = self
            .times
            .times_for(self.engine.as_ref(), &page.containers, &self.cancellation)
            .await;
        let node_id = self.node_id.clone();
        Ok(Response::new(ListContainersResponse {
            containers: page
                .containers
                .into_iter()
                .map(|container| {
                    let stats = self.stats.get(&container.container_id).unwrap_or_default();
                    let times = times
                        .get(&container.container_id)
                        .copied()
                        .unwrap_or_default();
                    ProtoContainerSummary {
                        state: domain_state_to_proto(container.state) as i32,
                        container_id: container.container_id,
                        display_name: container.display_name,
                        image_reference: container.image_reference,
                        status: container.status,
                        created_at_unix_seconds: container.created_at_seconds,
                        node_id: node_id.clone(),
                        cpu_percent: stats.cpu_percent,
                        memory_used_bytes: stats.memory_used_bytes,
                        memory_limit_bytes: stats.memory_limit_bytes,
                        started_at_unix_seconds: times.started_at_seconds,
                        finished_at_unix_seconds: times.finished_at_seconds,
                        published_ports: container.published_ports,
                        service_name: container.service_name,
                        stack_namespace: container.stack_namespace,
                        task_id: container.task_id,
                        networks: container.networks,
                    }
                })
                .collect(),
            next_page_token: page.next_page_token.unwrap_or_default(),
        }))
    }

    async fn container_action(
        &self,
        request: Request<ContainerActionRequest>,
    ) -> Result<Response<ContainerActionResponse>, Status> {
        request
            .extensions()
            .get::<AuthenticatedPeerIdentity>()
            .ok_or_else(|| {
                Status::unauthenticated("authenticated controller identity is required")
            })?;
        let inner = request.into_inner();
        if !is_valid_container_id(&inner.container_id) {
            return Err(Status::invalid_argument("invalid_container"));
        }
        let action = match ProtoContainerAction::try_from(inner.action) {
            Ok(ProtoContainerAction::Start) => ContainerAction::Start,
            Ok(ProtoContainerAction::Stop) => ContainerAction::Stop,
            Ok(ProtoContainerAction::Restart) => ContainerAction::Restart,
            Ok(ProtoContainerAction::Pause) => ContainerAction::Pause,
            Ok(ProtoContainerAction::Unpause) => ContainerAction::Unpause,
            Ok(ProtoContainerAction::Remove) => ContainerAction::Remove,
            Ok(ProtoContainerAction::Unspecified) | Err(_) => {
                return Err(Status::invalid_argument("invalid_action"));
            }
        };
        if inner
            .timeout_seconds
            .is_some_and(|seconds| seconds > MAX_STOP_TIMEOUT_SECONDS)
        {
            return Err(Status::invalid_argument("invalid_timeout"));
        }
        let options = ContainerActionOptions {
            force: inner.force,
            timeout_seconds: inner.timeout_seconds,
        };
        let result = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => {
                return Err(Status::cancelled("node agent is shutting down"));
            }
            result = self.engine.container_action(&inner.container_id, action, options, &self.cancellation) => result,
        };
        self.times.forget(&inner.container_id);
        result.map_err(|error| {
            warn!(%error, action = action.as_str(), "container action failed");
            action_error_to_status(&error)
        })?;
        Ok(Response::new(ContainerActionResponse {}))
    }
}

/// Stable codes in the status message; daemon detail stays in the log.
fn network_to_proto(network: NetworkSummary) -> ProtoNetworkSummary {
    ProtoNetworkSummary {
        builtin: network.builtin(),
        id: network.id,
        name: network.name,
        driver: network.driver,
        scope: network.scope,
        subnets: network.subnets,
        containers: network.containers,
        created_at_unix_seconds: network.created_at_seconds,
        labels: network.labels.into_iter().collect(),
        attachable: network.attachable,
        ingress: network.ingress,
        internal: network.internal,
    }
}

fn action_error_to_status(error: &EngineError) -> Status {
    match error.kind() {
        EngineErrorKind::NotFound => Status::not_found("not_found"),
        EngineErrorKind::NotModified => Status::failed_precondition("already_in_state"),
        EngineErrorKind::Conflict => Status::failed_precondition("container_running"),
        EngineErrorKind::InvalidInput => Status::invalid_argument("invalid_container"),
        EngineErrorKind::DeadlineExceeded => Status::deadline_exceeded("engine_timeout"),
        EngineErrorKind::Cancelled => Status::cancelled("cancelled"),
        _ => Status::unavailable("engine_error"),
    }
}

fn engine_error_to_status(error: &EngineError) -> Status {
    match error.kind() {
        EngineErrorKind::InvalidInput => Status::invalid_argument("pagination is invalid"),
        EngineErrorKind::Cancelled => Status::cancelled("container listing was cancelled"),
        EngineErrorKind::DeadlineExceeded => Status::deadline_exceeded("engine timed out"),
        EngineErrorKind::ConnectionUnavailable
        | EngineErrorKind::DaemonRejected
        | EngineErrorKind::DependencyFailure => Status::unavailable("engine is unavailable"),
        EngineErrorKind::UnsupportedCapability | EngineErrorKind::UnsupportedApiVersion => {
            Status::failed_precondition("container inventory is not supported")
        }
        EngineErrorKind::InvalidResponse | EngineErrorKind::InvalidConfiguration => {
            Status::internal("engine response was invalid")
        }
        EngineErrorKind::NotFound
        | EngineErrorKind::NotModified
        | EngineErrorKind::Conflict
        | EngineErrorKind::QuorumUnavailable
        | EngineErrorKind::VersionConflict => Status::unavailable("engine is unavailable"),
    }
}

fn domain_state_to_proto(state: ContainerState) -> ProtoContainerState {
    match state {
        ContainerState::Unknown => ProtoContainerState::Unknown,
        ContainerState::Created => ProtoContainerState::Created,
        ContainerState::Running => ProtoContainerState::Running,
        ContainerState::Paused => ProtoContainerState::Paused,
        ContainerState::Restarting => ProtoContainerState::Restarting,
        ContainerState::Removing => ProtoContainerState::Removing,
        ContainerState::Exited => ProtoContainerState::Exited,
        ContainerState::Dead => ProtoContainerState::Dead,
        ContainerState::Stopping => ProtoContainerState::Stopping,
    }
}

#[cfg(test)]
mod tests {
    use super::{ContainerEngine, NodeAgent, NodeAgentService};
    use async_trait::async_trait;
    use kontainer_agent_proto::v1::{
        ContainerState as ProtoContainerState, DescribeRequest, ListContainersRequest,
    };
    use kontainer_engine::{
        ContainerPage, ContainerPageRequest, ContainerState, ContainerSummary, EngineError,
        EngineErrorKind, EngineOperation,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio_util::sync::CancellationToken;
    use tonic::Request;

    struct AvailableEngine;

    #[async_trait]
    impl ContainerEngine for AvailableEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }
    }

    #[derive(Debug, Clone)]
    struct ScriptedEngine {
        pages: Vec<ContainerPage>,
        calls: Arc<AtomicUsize>,
        error: Option<(EngineErrorKind, EngineOperation)>,
    }

    #[async_trait]
    impl ContainerEngine for ScriptedEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }

        async fn list_containers(
            &self,
            _request: ContainerPageRequest,
            _cancellation: &CancellationToken,
        ) -> Result<ContainerPage, EngineError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some((kind, operation)) = self.error {
                return Err(EngineError::new(kind, operation));
            }
            let index = self.calls.load(Ordering::SeqCst) - 1;
            Ok(self.pages[index.min(self.pages.len() - 1)].clone())
        }
    }

    struct BlockingEngine {
        entered: Arc<tokio::sync::Semaphore>,
    }

    #[async_trait]
    impl ContainerEngine for BlockingEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }

        async fn list_containers(
            &self,
            _request: ContainerPageRequest,
            _cancellation: &CancellationToken,
        ) -> Result<ContainerPage, EngineError> {
            self.entered.add_permits(1);
            std::future::pending().await
        }
    }

    fn summary(id: &str, name: &str) -> ContainerSummary {
        ContainerSummary {
            container_id: id.to_owned(),
            display_name: name.to_owned(),
            image_reference: "nginx:latest".to_owned(),
            state: ContainerState::Running,
            status: "Up 3s".to_owned(),
            created_at_seconds: 1_700_000_000,
            ..ContainerSummary::default()
        }
    }

    fn authenticated<Service>(request: Service) -> Request<Service> {
        let mut request = Request::new(request);
        request
            .extensions_mut()
            .insert(kontainer_rpc_auth::AuthenticatedPeerIdentity::new_for_tests());
        request
    }

    type SeenAction = (String, kontainer_engine::ContainerAction, bool, Option<u32>);

    /// Running containers refuse start (304) and plain remove (409); unknown ids are 404.
    struct LifecycleEngine {
        running: String,
        seen: std::sync::Mutex<Vec<SeenAction>>,
    }

    #[async_trait]
    impl ContainerEngine for LifecycleEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }

        async fn container_action(
            &self,
            container_id: &str,
            action: kontainer_engine::ContainerAction,
            options: kontainer_engine::ContainerActionOptions,
            _cancellation: &CancellationToken,
        ) -> Result<(), EngineError> {
            use kontainer_engine::ContainerAction;
            self.seen.lock().unwrap().push((
                container_id.to_owned(),
                action,
                options.force,
                options.timeout_seconds,
            ));
            let error = |kind| Err(EngineError::new(kind, EngineOperation::ContainerAction));
            if container_id != self.running {
                return error(EngineErrorKind::NotFound);
            }
            match action {
                ContainerAction::Start => error(EngineErrorKind::NotModified),
                ContainerAction::Remove if !options.force => error(EngineErrorKind::Conflict),
                _ => Ok(()),
            }
        }
    }

    #[tokio::test]
    async fn container_actions_map_to_the_engine_and_stable_codes() {
        use kontainer_agent_proto::v1::{ContainerAction as Proto, ContainerActionRequest};

        let running = "a".repeat(64);
        let engine = Arc::new(LifecycleEngine {
            running: running.clone(),
            seen: std::sync::Mutex::new(Vec::new()),
        });
        let service = NodeAgentService::new("node-1", engine.clone(), CancellationToken::new());
        let request =
            |id: &str, action: Proto, force: bool, timeout: Option<u32>| ContainerActionRequest {
                container_id: id.to_owned(),
                action: action as i32,
                force,
                timeout_seconds: timeout,
            };

        assert!(
            service
                .container_action(authenticated(request(
                    &running,
                    Proto::Restart,
                    false,
                    Some(5)
                )))
                .await
                .is_ok()
        );
        assert!(
            service
                .container_action(authenticated(request(&running, Proto::Pause, false, None)))
                .await
                .is_ok()
        );
        assert!(
            service
                .container_action(authenticated(request(&running, Proto::Remove, true, None)))
                .await
                .is_ok()
        );
        for (req, code, message) in [
            (
                request(&running, Proto::Start, false, None),
                tonic::Code::FailedPrecondition,
                "already_in_state",
            ),
            (
                request(&running, Proto::Remove, false, None),
                tonic::Code::FailedPrecondition,
                "container_running",
            ),
            (
                request(&"b".repeat(64), Proto::Stop, false, None),
                tonic::Code::NotFound,
                "not_found",
            ),
            (
                request("short", Proto::Stop, false, None),
                tonic::Code::InvalidArgument,
                "invalid_container",
            ),
            (
                request(&running, Proto::Unspecified, false, None),
                tonic::Code::InvalidArgument,
                "invalid_action",
            ),
            (
                request(&running, Proto::Stop, false, Some(601)),
                tonic::Code::InvalidArgument,
                "invalid_timeout",
            ),
        ] {
            let error = service
                .container_action(authenticated(req))
                .await
                .unwrap_err();
            assert_eq!((error.code(), error.message()), (code, message));
        }
        let unauthenticated = service
            .container_action(Request::new(request(&running, Proto::Stop, false, None)))
            .await
            .unwrap_err();
        assert_eq!(unauthenticated.code(), tonic::Code::Unauthenticated);

        let seen = engine.seen.lock().unwrap();
        assert_eq!(seen.len(), 6, "invalid requests never reach the engine");
        assert_eq!(seen[0].1, kontainer_engine::ContainerAction::Restart);
        assert_eq!(seen[0].3, Some(5));
        assert!(seen[2].2, "force reaches the engine only when asked");
        assert!(!seen[4].2);
    }

    struct ImageEngine;

    #[async_trait]
    impl ContainerEngine for ImageEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }

        async fn list_images(
            &self,
            _: &CancellationToken,
        ) -> Result<Vec<kontainer_engine::ImageSummary>, EngineError> {
            Ok(vec![kontainer_engine::ImageSummary {
                image_id: format!("sha256:{}", "a".repeat(64)),
                repo_tags: vec!["nginx:1.27".to_owned()],
                size_bytes: 42,
                used_by: 3,
                ..Default::default()
            }])
        }

        async fn remove_image(
            &self,
            _image_id: &str,
            force: bool,
            _: &CancellationToken,
        ) -> Result<(), EngineError> {
            if force {
                Ok(())
            } else {
                Err(EngineError::new(
                    EngineErrorKind::Conflict,
                    EngineOperation::RemoveImage,
                ))
            }
        }

        async fn pull_image(
            &self,
            reference: &str,
            _: &CancellationToken,
        ) -> Result<kontainer_engine::PullStream, EngineError> {
            use kontainer_engine::{PullEvent, PullProgress};
            let mut events = vec![PullEvent::Progress(PullProgress {
                layer_id: Some("l1".to_owned()),
                status: "Downloading".to_owned(),
                current: Some(10),
                total: Some(20),
            })];
            if reference.contains("private") {
                events.push(PullEvent::Failed(
                    "unauthorized: authentication required".to_owned(),
                ));
            } else {
                events.push(PullEvent::Progress(PullProgress {
                    layer_id: Some("l1".to_owned()),
                    status: "Pull complete".to_owned(),
                    ..PullProgress::default()
                }));
            }
            Ok(Box::pin(futures_util::stream::iter(events)))
        }
    }

    #[tokio::test]
    async fn images_list_remove_and_pull_through_the_engine() {
        use futures_util::StreamExt as _;
        use kontainer_agent_proto::v1::{ListImagesRequest, PullImageRequest, RemoveImageRequest};

        let service =
            NodeAgentService::new("node-1", Arc::new(ImageEngine), CancellationToken::new());
        let listed = service
            .list_images(authenticated(ListImagesRequest {}))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(listed.images.len(), 1);
        assert_eq!(listed.images[0].used_by, 3);

        let id = format!("sha256:{}", "a".repeat(64));
        let in_use = service
            .remove_image(authenticated(RemoveImageRequest {
                image_id: id.clone(),
                force: false,
            }))
            .await
            .unwrap_err();
        assert_eq!(
            (in_use.code(), in_use.message()),
            (tonic::Code::FailedPrecondition, "image_in_use")
        );
        assert!(
            service
                .remove_image(authenticated(RemoveImageRequest {
                    image_id: id,
                    force: true
                }))
                .await
                .is_ok()
        );
        let invalid = service
            .remove_image(authenticated(RemoveImageRequest {
                image_id: "nginx".to_owned(),
                force: true,
            }))
            .await
            .unwrap_err();
        assert_eq!(invalid.code(), tonic::Code::InvalidArgument);

        let events: Vec<_> = service
            .pull_image(authenticated(PullImageRequest {
                reference: "alpine:3.20".to_owned(),
            }))
            .await
            .unwrap()
            .into_inner()
            .collect()
            .await;
        let statuses: Vec<_> = events
            .iter()
            .map(|event| event.as_ref().unwrap().status.clone())
            .collect();
        assert_eq!(statuses, vec!["Downloading", "Pull complete"]);
        assert_eq!(events[0].as_ref().unwrap().current, Some(10));

        let failed: Vec<_> = service
            .pull_image(authenticated(PullImageRequest {
                reference: "ghcr.io/private/x:1".to_owned(),
            }))
            .await
            .unwrap()
            .into_inner()
            .collect()
            .await;
        assert_eq!(
            failed.last().unwrap().as_ref().unwrap().error.as_deref(),
            Some("unauthorized: authentication required")
        );
        let rejected = service
            .pull_image(authenticated(PullImageRequest {
                reference: "has space".to_owned(),
            }))
            .await
            .err()
            .unwrap();
        assert_eq!(rejected.code(), tonic::Code::InvalidArgument);
        let anonymous = service
            .list_images(Request::new(ListImagesRequest {}))
            .await
            .unwrap_err();
        assert_eq!(anonymous.code(), tonic::Code::Unauthenticated);
    }

    /// One `pg-data` volume mounted by `db.1`; creates refuse existing names.
    struct VolumeEngine;

    #[async_trait]
    impl ContainerEngine for VolumeEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }

        async fn list_volumes(
            &self,
            _: &CancellationToken,
        ) -> Result<Vec<kontainer_engine::VolumeSummary>, EngineError> {
            Ok(vec![kontainer_engine::VolumeSummary {
                name: "pg-data".to_owned(),
                driver: "local".to_owned(),
                mountpoint: "/var/lib/docker/volumes/pg-data/_data".to_owned(),
                used_by: 1,
                users: vec!["db.1".to_owned()],
                ..Default::default()
            }])
        }

        async fn create_volume(
            &self,
            volume: &kontainer_engine::VolumeCreate,
            _: &CancellationToken,
        ) -> Result<kontainer_engine::VolumeSummary, EngineError> {
            if volume.name == "pg-data" {
                return Err(EngineError::new(
                    EngineErrorKind::Conflict,
                    EngineOperation::CreateVolume,
                ));
            }
            Ok(kontainer_engine::VolumeSummary {
                name: volume.name.clone(),
                driver: volume.driver.clone(),
                labels: volume.labels.clone(),
                ..Default::default()
            })
        }

        async fn remove_volume(
            &self,
            name: &str,
            _: &CancellationToken,
        ) -> Result<(), EngineError> {
            match name {
                "pg-data" => Err(EngineError::new(
                    EngineErrorKind::Conflict,
                    EngineOperation::RemoveVolume,
                )),
                "scratch" => Ok(()),
                _ => Err(EngineError::new(
                    EngineErrorKind::NotFound,
                    EngineOperation::RemoveVolume,
                )),
            }
        }
    }

    #[tokio::test]
    async fn volumes_list_with_cached_sizes_create_and_refuse_removal_while_mounted() {
        use kontainer_agent_proto::v1::{
            CreateVolumeRequest, ListVolumesRequest, RemoveVolumeRequest,
        };

        let service =
            NodeAgentService::new("node-1", Arc::new(VolumeEngine), CancellationToken::new());
        let listed = service
            .list_volumes(authenticated(ListVolumesRequest {}))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(listed.volumes[0].size_bytes, None, "no df yet");
        assert_eq!(listed.volumes[0].users, vec!["db.1"]);

        // Sizes come from the sampler's last df.
        let engine = Arc::new(crate::sampling::tests::FakeNodeEngine::two_containers());
        let sizes = crate::sampling::VolumeSizeCache::default();
        let mut sampler = crate::sampling::NodeSampler::new(
            engine,
            crate::sampling::ContainerStatsCache::default(),
        )
        .with_volume_sizes(sizes.clone());
        sampler
            .sample(std::time::Duration::from_secs(1), &CancellationToken::new())
            .await
            .unwrap();
        let sized = service.clone().with_volume_sizes(sizes);
        let listed = sized
            .list_volumes(authenticated(ListVolumesRequest {}))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(listed.volumes[0].size_bytes, Some(4096));

        let created = service
            .create_volume(authenticated(CreateVolumeRequest {
                name: "scratch".to_owned(),
                driver: "local".to_owned(),
                labels: std::collections::HashMap::from([("app".to_owned(), "billing".to_owned())]),
                driver_opts: std::collections::HashMap::new(),
            }))
            .await
            .unwrap()
            .into_inner()
            .volume
            .unwrap();
        assert_eq!(
            created.labels.get("app").map(String::as_str),
            Some("billing")
        );
        let exists = service
            .create_volume(authenticated(CreateVolumeRequest {
                name: "pg-data".to_owned(),
                driver: "local".to_owned(),
                ..Default::default()
            }))
            .await
            .unwrap_err();
        assert_eq!(
            (exists.code(), exists.message()),
            (tonic::Code::AlreadyExists, "volume_exists")
        );
        for name in ["a", "-bad", "has space"] {
            let invalid = service
                .create_volume(authenticated(CreateVolumeRequest {
                    name: name.to_owned(),
                    driver: "local".to_owned(),
                    ..Default::default()
                }))
                .await
                .unwrap_err();
            assert_eq!(invalid.code(), tonic::Code::InvalidArgument, "{name}");
        }

        let in_use = service
            .remove_volume(authenticated(RemoveVolumeRequest {
                name: "pg-data".to_owned(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(in_use.in_use_by, vec!["db.1"], "refused, with the names");
        let removed = service
            .remove_volume(authenticated(RemoveVolumeRequest {
                name: "scratch".to_owned(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert!(removed.in_use_by.is_empty());
        let missing = service
            .remove_volume(authenticated(RemoveVolumeRequest {
                name: "ghost".to_owned(),
            }))
            .await
            .unwrap_err();
        assert_eq!(missing.code(), tonic::Code::NotFound);
        let anonymous = service
            .remove_volume(Request::new(RemoveVolumeRequest {
                name: "scratch".to_owned(),
            }))
            .await
            .unwrap_err();
        assert_eq!(anonymous.code(), tonic::Code::Unauthenticated);
    }

    /// `bridge` (built-in), `app` with one endpoint, and `empty` with none.
    struct NetworkEngine {
        removed: std::sync::Mutex<Vec<String>>,
    }

    fn network(fill: char, name: &str, containers: u32) -> kontainer_engine::NetworkSummary {
        kontainer_engine::NetworkSummary {
            id: fill.to_string().repeat(64),
            name: name.to_owned(),
            driver: "bridge".to_owned(),
            scope: "local".to_owned(),
            subnets: vec!["172.30.0.0/16".to_owned()],
            containers,
            ..Default::default()
        }
    }

    #[async_trait]
    impl ContainerEngine for NetworkEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }

        async fn list_networks(
            &self,
            _: &CancellationToken,
        ) -> Result<Vec<kontainer_engine::NetworkSummary>, EngineError> {
            Ok(vec![
                network('a', "bridge", 0),
                network('b', "app", 1),
                network('c', "empty", 0),
            ])
        }

        async fn inspect_network(
            &self,
            id: &str,
            _: &CancellationToken,
        ) -> Result<kontainer_engine::NetworkDetail, EngineError> {
            let listed = [
                network('a', "bridge", 0),
                network('b', "app", 1),
                network('c', "empty", 0),
            ];
            let Some(found) = listed.into_iter().find(|network| network.id == id) else {
                return Err(EngineError::new(
                    EngineErrorKind::NotFound,
                    EngineOperation::InspectNetwork,
                ));
            };
            let endpoints = if found.name == "app" {
                vec![kontainer_engine::NetworkEndpoint {
                    container_id: "d".repeat(64),
                    name: "web.1".to_owned(),
                }]
            } else {
                Vec::new()
            };
            Ok(kontainer_engine::NetworkDetail {
                network: found,
                endpoints,
            })
        }

        async fn create_network(
            &self,
            network: &kontainer_engine::NetworkCreate,
            _: &CancellationToken,
        ) -> Result<String, EngineError> {
            assert_eq!(network.driver, "bridge", "agents only make bridges");
            Ok("e".repeat(64))
        }

        async fn remove_network(&self, id: &str, _: &CancellationToken) -> Result<(), EngineError> {
            self.removed.lock().unwrap().push(id.to_owned());
            Ok(())
        }
    }

    #[tokio::test]
    async fn networks_list_inspect_create_and_refuse_removal_with_endpoints() {
        use kontainer_agent_proto::v1::{
            CreateNetworkRequest, InspectNetworkRequest, ListNetworksRequest, RemoveNetworkRequest,
        };

        let engine = Arc::new(NetworkEngine {
            removed: std::sync::Mutex::new(Vec::new()),
        });
        let service = NodeAgentService::new("node-1", engine.clone(), CancellationToken::new());
        let listed = service
            .list_networks(authenticated(ListNetworksRequest {}))
            .await
            .unwrap()
            .into_inner();
        let flags: Vec<_> = listed
            .networks
            .iter()
            .map(|network| (network.name.as_str(), network.builtin, network.containers))
            .collect();
        assert_eq!(
            flags,
            vec![("bridge", true, 0), ("app", false, 1), ("empty", false, 0)]
        );

        let detail = service
            .inspect_network(authenticated(InspectNetworkRequest { id: "b".repeat(64) }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(detail.endpoints[0].name, "web.1");
        let missing = service
            .inspect_network(authenticated(InspectNetworkRequest { id: "f".repeat(64) }))
            .await
            .unwrap_err();
        assert_eq!(missing.code(), tonic::Code::NotFound);

        let in_use = service
            .remove_network(authenticated(RemoveNetworkRequest { id: "b".repeat(64) }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(in_use.in_use_by, vec!["web.1"]);
        let builtin = service
            .remove_network(authenticated(RemoveNetworkRequest { id: "a".repeat(64) }))
            .await
            .unwrap_err();
        assert_eq!(
            (builtin.code(), builtin.message()),
            (tonic::Code::InvalidArgument, "builtin_network")
        );
        let removed = service
            .remove_network(authenticated(RemoveNetworkRequest { id: "c".repeat(64) }))
            .await
            .unwrap()
            .into_inner();
        assert!(removed.in_use_by.is_empty());
        assert_eq!(
            *engine.removed.lock().unwrap(),
            vec!["c".repeat(64)],
            "only the empty one"
        );

        let created = service
            .create_network(authenticated(CreateNetworkRequest {
                name: "local-net".to_owned(),
                subnet: Some("10.9.0.0/24".to_owned()),
                labels: std::collections::HashMap::new(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(created.id, "e".repeat(64));
        for (name, subnet) in [
            ("bridge", None),
            ("-bad", None),
            ("ok", Some("10.9.0.1/24")),
        ] {
            let invalid = service
                .create_network(authenticated(CreateNetworkRequest {
                    name: name.to_owned(),
                    subnet: subnet.map(str::to_owned),
                    labels: std::collections::HashMap::new(),
                }))
                .await
                .unwrap_err();
            assert_eq!(invalid.code(), tonic::Code::InvalidArgument, "{name}");
        }
    }

    #[tokio::test]
    async fn describe_does_not_run_for_an_unauthenticated_caller() {
        let service = NodeAgentService::new(
            "node-1",
            Arc::new(AvailableEngine),
            CancellationToken::new(),
        );

        let error = service
            .describe(Request::new(DescribeRequest {}))
            .await
            .unwrap_err();

        assert_eq!(error.code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn list_containers_rejects_unauthenticated_callers_before_validation() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = NodeAgentService::new(
            "node-1",
            Arc::new(ScriptedEngine {
                pages: vec![ContainerPage {
                    containers: vec![],
                    next_page_token: None,
                }],
                calls: calls.clone(),
                error: None,
            }),
            CancellationToken::new(),
        );

        let error = service
            .list_containers(Request::new(ListContainersRequest {
                page_size: 0,
                page_token: String::new(),
            }))
            .await
            .unwrap_err();

        assert_eq!(error.code(), tonic::Code::Unauthenticated);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn list_containers_validates_pagination_without_engine_access() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = NodeAgentService::new(
            "node-1",
            Arc::new(ScriptedEngine {
                pages: vec![ContainerPage {
                    containers: vec![],
                    next_page_token: None,
                }],
                calls: calls.clone(),
                error: None,
            }),
            CancellationToken::new(),
        );

        for request in [
            ListContainersRequest {
                page_size: 501,
                page_token: String::new(),
            },
            ListContainersRequest {
                page_size: 10,
                page_token: "not-a-cursor".to_owned(),
            },
            ListContainersRequest {
                page_size: 10,
                page_token: "v1:short".to_owned(),
            },
        ] {
            let error = service
                .list_containers(authenticated(request))
                .await
                .unwrap_err();
            assert_eq!(error.code(), tonic::Code::InvalidArgument);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn list_containers_returns_configured_provenance_and_cursors() {
        let first_id = "a".repeat(64);
        let second_id = "b".repeat(64);
        let cursor = format!("v1:{first_id}");
        let calls = Arc::new(AtomicUsize::new(0));
        let service = NodeAgentService::new(
            "node-9",
            Arc::new(ScriptedEngine {
                pages: vec![
                    ContainerPage {
                        containers: vec![summary(&first_id, "web")],
                        next_page_token: Some(cursor.clone()),
                    },
                    ContainerPage {
                        containers: vec![summary(&second_id, "worker")],
                        next_page_token: None,
                    },
                ],
                calls: calls.clone(),
                error: None,
            }),
            CancellationToken::new(),
        );

        let first = service
            .list_containers(authenticated(ListContainersRequest {
                page_size: 1,
                page_token: String::new(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(first.containers.len(), 1);
        assert_eq!(first.containers[0].container_id, first_id);
        assert_eq!(first.containers[0].node_id, "node-9");
        assert_eq!(
            first.containers[0].state,
            ProtoContainerState::Running as i32
        );
        assert_eq!(first.next_page_token, cursor);

        let second = service
            .list_containers(authenticated(ListContainersRequest {
                page_size: 1,
                page_token: cursor,
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(second.containers.len(), 1);
        assert_eq!(second.containers[0].container_id, second_id);
        assert_eq!(second.containers[0].node_id, "node-9");
        assert!(second.next_page_token.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn list_containers_maps_engine_failures_to_stable_statuses() {
        for (kind, code) in [
            (EngineErrorKind::InvalidInput, tonic::Code::InvalidArgument),
            (
                EngineErrorKind::DeadlineExceeded,
                tonic::Code::DeadlineExceeded,
            ),
            (
                EngineErrorKind::ConnectionUnavailable,
                tonic::Code::Unavailable,
            ),
            (EngineErrorKind::DaemonRejected, tonic::Code::Unavailable),
            (
                EngineErrorKind::UnsupportedCapability,
                tonic::Code::FailedPrecondition,
            ),
            (EngineErrorKind::InvalidResponse, tonic::Code::Internal),
        ] {
            let service = NodeAgentService::new(
                "node-1",
                Arc::new(ScriptedEngine {
                    pages: vec![],
                    calls: Arc::new(AtomicUsize::new(0)),
                    error: Some((kind, EngineOperation::ListContainers)),
                }),
                CancellationToken::new(),
            );
            let error = service
                .list_containers(authenticated(ListContainersRequest {
                    page_size: 10,
                    page_token: String::new(),
                }))
                .await
                .unwrap_err();
            assert_eq!(error.code(), code, "{kind:?}");
            assert!(!error.message().contains("secret-canary"));
        }
    }

    #[tokio::test]
    async fn list_containers_cancellation_wins_without_detached_work() {
        use std::time::Duration;

        let entered = Arc::new(tokio::sync::Semaphore::new(0));
        let service = NodeAgentService::new(
            "node-1",
            Arc::new(BlockingEngine {
                entered: entered.clone(),
            }),
            CancellationToken::new(),
        );
        let cancellation = service.cancellation.clone();
        let call = tokio::spawn(async move {
            service
                .list_containers(authenticated(ListContainersRequest {
                    page_size: 10,
                    page_token: String::new(),
                }))
                .await
        });

        entered.acquire().await.unwrap().forget();
        cancellation.cancel();
        let error = tokio::time::timeout(Duration::from_secs(2), call)
            .await
            .expect("in-flight listing did not observe shutdown")
            .expect("listing task panicked")
            .unwrap_err();
        assert_eq!(error.code(), tonic::Code::Cancelled);
    }
}
