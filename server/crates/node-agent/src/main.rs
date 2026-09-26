#![forbid(unsafe_code)]

use std::{env, error::Error, ffi::OsString, sync::Arc};

use kontainer_engine::{LocalDockerEngine, LocalEngineOptions};
use kontainer_node_agent::{
    AgentConfig, AgentShutdown, NodeAgentService, serve_node_agent_rpc, supervise_node_agent_server,
};
use tokio::net::TcpListener;
use tokio_util::task::TaskTracker;
use tracing_subscriber::EnvFilter;

fn is_version_request(arguments: &[OsString]) -> bool {
    arguments.len() == 1 && matches!(arguments[0].to_str(), Some("--version" | "-V" | "version"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if is_version_request(&arguments) {
        println!("kontainer-node-agent {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    let config = AgentConfig::from_environment()?;
    let listener = TcpListener::bind(config.binding).await?;
    let shutdown = AgentShutdown::new(config.shutdown_grace);
    let connections = TaskTracker::new();
    let requests = TaskTracker::new();
    let (failures, task_failures) = tokio::sync::mpsc::unbounded_channel();
    let service = NodeAgentService::new(
        config.node_id,
        Arc::new(LocalDockerEngine::connect_for_node(
            LocalEngineOptions::default(),
        )?),
        shutdown.force(),
    );
    let server = serve_node_agent_rpc(
        listener,
        config.cluster_id,
        config.tls_files,
        service,
        shutdown.draining(),
        shutdown.force(),
        connections.clone(),
        requests.clone(),
        failures,
    )?;
    supervise_node_agent_server(
        server,
        shutdown,
        connections,
        requests,
        task_failures,
        shutdown_signal(),
    )
    .await
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

#[cfg(test)]
mod tests {
    use super::is_version_request;
    use std::ffi::OsString;

    #[test]
    fn version_command_does_not_require_runtime_configuration() {
        for argument in ["--version", "-V", "version"] {
            assert!(is_version_request(&[OsString::from(argument)]));
        }

        assert!(!is_version_request(&[]));
        assert!(!is_version_request(&[
            OsString::from("--version"),
            OsString::from("unexpected"),
        ]));
    }
}
