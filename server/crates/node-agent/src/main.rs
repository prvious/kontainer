#![forbid(unsafe_code)]

use std::{env, error::Error, ffi::OsString, sync::Arc};

use kontainer_engine::{ContainerEngine, LocalDockerEngine, LocalEngineOptions};
use kontainer_node_agent::{
    AgentConfig, AgentFacts, AgentShutdown, CheckInTiming, ContainerStatsCache, MtlsControllerLink,
    NodeAgentService, NodeSampler, VolumeSizeCache, run_check_in, serve_node_agent_rpc,
    supervise_node_agent_server,
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
    let controller = Arc::new(MtlsControllerLink::new(
        &config.controller_rpc_endpoint,
        &config.controller_server_name,
        &config.cluster_id,
        &config.tls_files,
    )?);
    let listener = TcpListener::bind(config.binding).await?;
    let shutdown = AgentShutdown::new(config.shutdown_grace);
    let connections = TaskTracker::new();
    let requests = TaskTracker::new();
    let (failures, task_failures) = tokio::sync::mpsc::unbounded_channel();
    let engine = Arc::new(LocalDockerEngine::connect_for_node(LocalEngineOptions {
        host: config.docker_host.clone(),
        ..LocalEngineOptions::default()
    })?);
    // A failed ping is logged, not fatal: Describe reports Docker health and
    // the agent keeps serving while the daemon restarts.
    match engine.is_available().await {
        Ok(true) => tracing::info!(docker.host = %config.docker_host, "docker ping succeeded"),
        Ok(false) | Err(_) => {
            tracing::warn!(docker.host = %config.docker_host, "docker ping failed");
        }
    }
    let stats = ContainerStatsCache::default();
    let volume_sizes = VolumeSizeCache::default();
    let facts = AgentFacts {
        node_id: config.node_id.clone(),
        advertise_address: config.advertise_address.clone(),
    };
    // Tracked with in-flight requests: draining cancels the loop and the grace deadline joins it.
    requests.spawn(run_check_in(
        controller,
        NodeSampler::new(engine.clone(), stats.clone()).with_volume_sizes(volume_sizes.clone()),
        facts,
        CheckInTiming::default(),
        shutdown.draining(),
    ));
    let service = NodeAgentService::new(config.node_id, engine, shutdown.force())
        .with_stats(stats)
        .with_volume_sizes(volume_sizes);
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
