#![forbid(unsafe_code)]

use std::{env, error::Error, fs, net::SocketAddr, path::PathBuf, sync::Arc};

use kontainer_agent_proto::v1::{
    DescribeRequest, DescribeResponse,
    node_agent_server::{NodeAgent, NodeAgentServer},
};
use kontainer_engine::{ContainerEngine, LocalDockerEngine, LocalEngineOptions};
use tonic::{Request, Response, Status, transport::Server};
use tracing::warn;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone)]
struct AgentConfig {
    node_id: String,
    binding: SocketAddr,
    tls: TlsFiles,
}

#[derive(Debug, Clone)]
struct TlsFiles {
    certificate_path: PathBuf,
    private_key_path: PathBuf,
    client_ca_path: PathBuf,
}

impl AgentConfig {
    fn from_environment() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            node_id: required_environment_variable("KONTAINER_NODE_ID")?,
            binding: environment_variable("KONTAINER_NODE_RPC_BINDING", "0.0.0.0:50052").parse()?,
            tls: TlsFiles {
                certificate_path: required_environment_variable(
                    "KONTAINER_NODE_RPC_TLS_CERTIFICATE_PATH",
                )?
                .into(),
                private_key_path: required_environment_variable(
                    "KONTAINER_NODE_RPC_TLS_PRIVATE_KEY_PATH",
                )?
                .into(),
                client_ca_path: required_environment_variable(
                    "KONTAINER_NODE_RPC_TLS_CLIENT_CA_PATH",
                )?
                .into(),
            },
        })
    }
}

fn required_environment_variable(name: &str) -> Result<String, Box<dyn Error>> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("required environment variable {name} is not set").into())
}

fn environment_variable(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}

#[derive(Clone)]
struct NodeAgentService {
    node_id: String,
    engine: Arc<dyn ContainerEngine>,
}

#[tonic::async_trait]
impl NodeAgent for NodeAgentService {
    async fn describe(
        &self,
        _request: Request<DescribeRequest>,
    ) -> Result<Response<DescribeResponse>, Status> {
        let docker_connected = match self.engine.is_available().await {
            Ok(is_available) => is_available,
            Err(error) => {
                warn!(%error, "Docker health check failed");
                false
            }
        };

        Ok(Response::new(DescribeResponse {
            node_id: self.node_id.clone(),
            agent_version: env!("CARGO_PKG_VERSION").to_owned(),
            architecture: std::env::consts::ARCH.to_owned(),
            docker_connected,
        }))
    }
}

fn tls_config(files: &TlsFiles) -> Result<tonic::transport::ServerTlsConfig, Box<dyn Error>> {
    let certificate = fs::read(&files.certificate_path)?;
    let private_key = fs::read(&files.private_key_path)?;
    let client_ca = fs::read(&files.client_ca_path)?;

    Ok(tonic::transport::ServerTlsConfig::new()
        .identity(tonic::transport::Identity::from_pem(
            certificate,
            private_key,
        ))
        .client_ca_root(tonic::transport::Certificate::from_pem(client_ca)))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    let config = AgentConfig::from_environment()?;
    let service = NodeAgentService {
        node_id: config.node_id,
        engine: Arc::new(LocalDockerEngine::connect_for_node(
            LocalEngineOptions::default(),
        )?),
    };

    Server::builder()
        .tls_config(tls_config(&config.tls)?)?
        .add_service(NodeAgentServer::new(service))
        .serve_with_shutdown(config.binding, shutdown_signal())
        .await?;

    Ok(())
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
    use super::{ContainerEngine, NodeAgent, NodeAgentService};
    use async_trait::async_trait;
    use kontainer_agent_proto::v1::DescribeRequest;
    use kontainer_engine::EngineError;
    use std::sync::Arc;
    use tonic::Request;

    struct AvailableEngine;

    #[async_trait]
    impl ContainerEngine for AvailableEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }
    }

    #[tokio::test]
    async fn describe_reports_node_identity_and_engine_health() {
        let service = NodeAgentService {
            node_id: "node-1".to_owned(),
            engine: Arc::new(AvailableEngine),
        };

        let response = service
            .describe(Request::new(DescribeRequest {}))
            .await
            .unwrap()
            .into_inner();

        assert_eq!(response.node_id, "node-1");
        assert_eq!(response.agent_version, env!("CARGO_PKG_VERSION"));
        assert!(response.docker_connected);
        assert!(!response.architecture.is_empty());
    }
}
