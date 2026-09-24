use std::future::Future;

use hyper::{body::Incoming, server::conn::http2};
use hyper_util::{rt::TokioIo, service::TowerToHyperService};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedSender;
use tokio_rustls::TlsAcceptor;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tonic::{service::Routes, transport::server::Connected};
use tower::{ServiceExt, service_fn};

#[derive(Clone)]
struct TrackedExecutor {
    tasks: TaskTracker,
    force: CancellationToken,
    failures: UnboundedSender<&'static str>,
}

/// Keep the join result of every tracked task so a panic can start draining.
pub fn spawn_tracked<F>(
    tasks: &TaskTracker,
    failures: &UnboundedSender<&'static str>,
    kind: &'static str,
    future: F,
) where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let handle = tasks.spawn(future);
    let failures = failures.clone();
    tasks.spawn(async move {
        if handle.await.is_err() {
            let _ = failures.send(kind);
        }
    });
}

impl<F> hyper::rt::Executor<F> for TrackedExecutor
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    fn execute(&self, future: F) {
        let force = self.force.clone();
        spawn_tracked(&self.tasks, &self.failures, "rpc request", async move {
            tokio::select! {
                _ = force.cancelled() => {}
                _ = future => {}
            }
        });
    }
}

/// A single owned HTTP/2 connection, including its TLS handshake and peer
/// certificate extension used by `PeerIdentityInterceptor`.
pub async fn serve_rpc_connection(
    socket: TcpStream,
    acceptor: TlsAcceptor,
    routes: Routes,
    draining: CancellationToken,
    force: CancellationToken,
    request_tasks: TaskTracker,
    failures: UnboundedSender<&'static str>,
) {
    let stream = tokio::select! {
        biased;
        _ = draining.cancelled() => return,
        accepted = acceptor.accept(socket) => match accepted {
            Ok(stream) => stream,
            Err(_) => return,
        },
    };
    let connection_info = stream.connect_info();
    let service = service_fn(move |mut request: hyper::Request<Incoming>| {
        request.extensions_mut().insert(connection_info.clone());
        let routes = routes.clone();
        async move { routes.oneshot(request).await }
    });
    let connection = http2::Builder::new(TrackedExecutor {
        tasks: request_tasks,
        force,
        failures,
    })
    .serve_connection(TokioIo::new(stream), TowerToHyperService::new(service));
    tokio::pin!(connection);
    tokio::select! {
        biased;
        _ = draining.cancelled() => {
            connection.as_mut().graceful_shutdown();
            let _ = connection.await;
        }
        _ = &mut connection => {}
    }
}

/// Accept only until draining; the caller owns the trackers and their final wait.
#[allow(clippy::too_many_arguments)] // Lifecycle and task ownership stay explicit at this boundary.
pub async fn serve_rpc_listener(
    listener: TcpListener,
    acceptor: TlsAcceptor,
    routes: Routes,
    draining: CancellationToken,
    force: CancellationToken,
    connections: TaskTracker,
    request_tasks: TaskTracker,
    failures: UnboundedSender<&'static str>,
) -> std::io::Result<()> {
    loop {
        let (socket, _) = tokio::select! {
            biased;
            _ = draining.cancelled() => return Ok(()),
            accepted = listener.accept() => accepted?,
        };
        let accepted = acceptor.clone();
        let routes = routes.clone();
        let drain = draining.clone();
        let force_connection = force.clone();
        let request_tasks = request_tasks.clone();
        let connection_failures = failures.clone();
        spawn_tracked(&connections, &failures, "rpc connection", async move {
            tokio::select! {
                biased;
                _ = force_connection.cancelled() => {}
                _ = serve_rpc_connection(socket, accepted, routes, drain, force_connection.clone(), request_tasks, connection_failures) => {}
            }
        });
    }
}
