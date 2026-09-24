use std::{error::Error, future::Future, sync::Arc, time::Instant};

use kontainer_agent_proto::v1::node_agent_server::NodeAgentServer;
use kontainer_lifecycle::{DrainClock, ShutdownGrace};
use kontainer_rpc_auth::{MutualTlsIdentityFiles, PeerIdentityInterceptor, WorkloadKind};
use tokio::{
    net::TcpListener,
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tonic::{
    Status,
    service::{Interceptor, Routes},
};

use crate::NodeAgentService;

#[derive(Clone)]
pub struct AgentShutdown {
    draining: CancellationToken,
    force: CancellationToken,
    clock: Arc<DrainClock>,
}

impl AgentShutdown {
    pub fn new(grace: ShutdownGrace) -> Self {
        Self {
            draining: CancellationToken::new(),
            force: CancellationToken::new(),
            clock: Arc::new(DrainClock::new(grace)),
        }
    }

    pub fn begin_drain(&self) -> Instant {
        let deadline = self.clock.begin();
        self.draining.cancel();
        deadline
    }

    pub fn draining(&self) -> CancellationToken {
        self.draining.clone()
    }

    pub fn force(&self) -> CancellationToken {
        self.force.clone()
    }
}

/// Validate TLS eagerly; polling the returned future begins RPC admission.
#[allow(clippy::too_many_arguments)] // Explicit listener, identity, lifecycle, and task ownership.
pub fn serve_node_agent_rpc(
    listener: TcpListener,
    cluster_id: String,
    tls_files: MutualTlsIdentityFiles,
    service: NodeAgentService,
    cancellation: CancellationToken,
    force: CancellationToken,
    connections: TaskTracker,
    request_tasks: TaskTracker,
    failures: UnboundedSender<&'static str>,
) -> Result<impl Future<Output = std::io::Result<()>> + Send, Box<dyn Error + Send + Sync>> {
    let mut peer_identity = PeerIdentityInterceptor::new(cluster_id, WorkloadKind::Controller)?;
    let acceptor = tls_files.server_acceptor()?;
    let admission = cancellation.clone();
    let service = NodeAgentServer::with_interceptor(service, move |request| {
        let request = peer_identity.call(request)?;
        if admission.is_cancelled() {
            Err(Status::unavailable("node agent is draining"))
        } else {
            Ok(request)
        }
    });
    Ok(kontainer_rpc_auth::serve_rpc_listener(
        listener,
        acceptor,
        Routes::new(service).prepare(),
        cancellation,
        force,
        connections,
        request_tasks,
        failures,
    ))
}

pub async fn supervise_node_agent_server(
    server: impl Future<Output = std::io::Result<()>> + Send + 'static,
    shutdown: AgentShutdown,
    connections: TaskTracker,
    requests: TaskTracker,
    mut task_failures: UnboundedReceiver<&'static str>,
    signal: impl Future<Output = ()>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut servers = JoinSet::new();
    servers.spawn(server);
    let draining = shutdown.draining();
    let mut failure = tokio::select! {
        biased;
        _ = draining.cancelled() => None,
        _ = signal => None,
        completion = servers.join_next() => Some(match completion {
            Some(Ok(Ok(()))) => "node agent RPC server stopped unexpectedly".to_owned(),
            Some(Ok(Err(error))) => format!("node agent RPC server failed: {error}"),
            Some(Err(error)) => format!("node agent RPC server task failed: {error}"),
            None => "node agent RPC server stopped unexpectedly".to_owned(),
        }),
        kind = task_failures.recv() => kind.map(|kind| format!("{kind} task failed")),
    };
    let deadline = shutdown.begin_drain();
    let graceful = async {
        while let Some(completion) = servers.join_next().await {
            if failure.is_none() {
                failure = match completion {
                    Ok(Ok(())) => None,
                    Ok(Err(error)) => Some(format!("node agent RPC server failed: {error}")),
                    Err(error) => Some(format!("node agent RPC server task failed: {error}")),
                };
            }
        }
        connections.close();
        requests.close();
        tokio::join!(connections.wait(), requests.wait());
    };
    if tokio::time::timeout_at(deadline.into(), graceful)
        .await
        .is_err()
    {
        tracing::error!(
            role = "node-agent",
            rpc_connections = connections.len(),
            rpc_requests = requests.len(),
            listener_tasks = servers.len(),
            "shutdown deadline exceeded"
        );
        shutdown.force.cancel();
        servers.shutdown().await;
        connections.close();
        requests.close();
        tokio::join!(connections.wait(), requests.wait());
        return Err(failure
            .unwrap_or_else(|| "node agent shutdown deadline exceeded".to_owned())
            .into());
    }
    if let Some(failure) = failure {
        return Err(failure.into());
    }
    if let Ok(kind) = task_failures.try_recv() {
        return Err(format!("{kind} task failed").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AgentShutdown, supervise_node_agent_server};
    use kontainer_lifecycle::ShutdownGrace;
    use std::{future::pending, io, time::Duration};
    use tokio::sync::oneshot;
    use tokio_util::task::TaskTracker;

    #[tokio::test]
    async fn signal_drains_short_work_without_forcing_it() {
        let shutdown = AgentShutdown::new("250ms".parse::<ShutdownGrace>().unwrap());
        let connections = TaskTracker::new();
        let requests = TaskTracker::new();
        let (release, work) = oneshot::channel::<()>();
        requests.spawn(async move { work.await.unwrap() });
        let draining = shutdown.draining();
        let server = async move {
            draining.cancelled().await;
            Ok(())
        };
        let (signal, received) = oneshot::channel::<()>();
        let (_failures, task_failures) = tokio::sync::mpsc::unbounded_channel();
        let task_shutdown = shutdown.clone();
        let task_requests = requests.clone();
        let task = tokio::spawn(async move {
            supervise_node_agent_server(
                server,
                task_shutdown,
                connections,
                task_requests,
                task_failures,
                async {
                    received.await.unwrap();
                },
            )
            .await
        });
        signal.send(()).unwrap();
        tokio::time::timeout(
            Duration::from_secs(1),
            shutdown.draining().cancelled_owned(),
        )
        .await
        .unwrap();
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(!shutdown.force().is_cancelled());
        assert_eq!(requests.len(), 0);
    }

    #[tokio::test]
    async fn listener_failure_starts_one_bounded_drain() {
        let shutdown = AgentShutdown::new("30ms".parse::<ShutdownGrace>().unwrap());
        let connections = TaskTracker::new();
        let requests = TaskTracker::new();
        let force = shutdown.force();
        let (_failures, task_failures) = tokio::sync::mpsc::unbounded_channel();
        requests.spawn(async move { force.cancelled().await });
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            supervise_node_agent_server(
                async { Err(io::Error::other("listener failed")) },
                shutdown.clone(),
                connections,
                requests.clone(),
                task_failures,
                pending(),
            ),
        )
        .await
        .unwrap();
        assert!(result.unwrap_err().to_string().contains("listener failed"));
        assert!(shutdown.draining().is_cancelled());
        assert!(shutdown.force().is_cancelled());
        assert_eq!(requests.len(), 0);
    }
}
