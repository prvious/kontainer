#![forbid(unsafe_code)]

use std::{error::Error, sync::Arc};

use kontainer_engine::{LocalDockerEngine, LocalEngineOptions};
use kontainer_node_agent::{AgentConfig, NodeAgentService, serve_node_agent_rpc};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    let config = AgentConfig::from_environment()?;
    let listener = TcpListener::bind(config.binding).await?;
    let cancellation = CancellationToken::new();
    let service = NodeAgentService::new(
        config.node_id,
        Arc::new(LocalDockerEngine::connect_for_node(
            LocalEngineOptions::default(),
        )?),
        cancellation.clone(),
    );
    let server = serve_node_agent_rpc(
        listener,
        config.cluster_id,
        config.tls_files,
        service,
        cancellation.clone(),
    );
    tokio::pin!(server);

    let result = tokio::select! {
        result = &mut server => result,
        _ = shutdown_signal() => {
            cancellation.cancel();
            server.await
        }
    };
    cancellation.cancel();
    result
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install SIGTERM handler");

        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                result.expect("failed to install Ctrl+C handler");
            }
            _ = terminate.recv() => {}
        }
    }

    #[cfg(not(unix))]
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
}
