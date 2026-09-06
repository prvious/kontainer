use std::{net::SocketAddr, path::PathBuf};

use axum::Router;
use kontainer_rpc_auth::MutualTlsIdentityFiles;
use loco_rs::{
    Error, Result,
    app::AppContext,
    boot::{ServeParams, shutdown_signal},
};
use serde::Deserialize;
use tokio::{net::TcpListener, task::JoinSet};
use tokio_util::sync::CancellationToken;

use crate::rpc::serve_controller_rpc;

#[derive(Debug, Clone, Deserialize)]
struct ControllerSettings {
    agent_rpc: AgentRpcSettings,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentRpcSettings {
    enabled: bool,
    binding: SocketAddr,
    cluster_id: String,
    tls: AgentRpcTlsSettings,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentRpcTlsSettings {
    certificate_path: PathBuf,
    private_key_path: PathBuf,
    client_ca_path: PathBuf,
}

#[derive(Debug, Clone, Copy)]
enum ServerKind {
    Http,
    AgentRpc,
}

impl std::fmt::Display for ServerKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http => formatter.write_str("HTTP"),
            Self::AgentRpc => formatter.write_str("agent RPC"),
        }
    }
}

pub async fn serve(router: Router, ctx: &AppContext, params: &ServeParams) -> Result<()> {
    let settings = controller_settings(ctx)?;
    let http_listener = TcpListener::bind(format!("{}:{}", params.binding, params.port)).await?;
    let agent_rpc = if settings.agent_rpc.enabled {
        Some(BoundAgentRpcServer {
            listener: TcpListener::bind(settings.agent_rpc.binding).await?,
            cluster_id: settings.agent_rpc.cluster_id,
            tls_files: settings.agent_rpc.tls.into(),
        })
    } else {
        None
    };
    let cancellation = CancellationToken::new();
    let servers =
        supervise_controller_servers(router, http_listener, agent_rpc, cancellation.clone());
    tokio::pin!(servers);

    tokio::select! {
        result = &mut servers => result,
        _ = shutdown_signal() => {
            cancellation.cancel();
            servers.await
        }
    }
}

pub struct BoundAgentRpcServer {
    pub listener: TcpListener,
    pub cluster_id: String,
    pub tls_files: MutualTlsIdentityFiles,
}

pub async fn supervise_controller_servers(
    router: Router,
    http_listener: TcpListener,
    agent_rpc: Option<BoundAgentRpcServer>,
    cancellation: CancellationToken,
) -> Result<()> {
    let mut servers = JoinSet::new();

    let http_cancellation = cancellation.clone();
    servers.spawn(async move {
        axum::serve(
            http_listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(http_cancellation.cancelled_owned())
        .await
        .map_err(Error::wrap)?;

        Ok(ServerKind::Http)
    });

    if let Some(agent_rpc) = agent_rpc {
        let rpc_cancellation = cancellation.clone();
        servers.spawn(async move {
            serve_controller_rpc(
                agent_rpc.listener,
                agent_rpc.cluster_id,
                agent_rpc.tls_files,
                rpc_cancellation,
            )
            .await
            .map_err(|error| Error::Message(format!("agent RPC server failed: {error}")))?;

            Ok(ServerKind::AgentRpc)
        });
    }

    let first_completion = tokio::select! {
        biased;
        _ = cancellation.cancelled() => None,
        completion = servers.join_next() => completion,
    };

    let mut failure = first_completion.map(unexpected_server_completion);
    cancellation.cancel();

    while let Some(completion) = servers.join_next().await {
        if failure.is_none()
            && let Err(error) = completed_server(completion)
        {
            failure = Some(error);
        }
    }

    failure.map_or(Ok(()), Err)
}

fn controller_settings(ctx: &AppContext) -> Result<ControllerSettings> {
    let settings = ctx
        .config
        .settings
        .clone()
        .ok_or_else(|| Error::string("controller settings are missing"))?;

    serde_json::from_value(settings).map_err(Error::wrap)
}

impl From<AgentRpcTlsSettings> for MutualTlsIdentityFiles {
    fn from(settings: AgentRpcTlsSettings) -> Self {
        Self::new(
            settings.certificate_path,
            settings.private_key_path,
            settings.client_ca_path,
        )
    }
}

fn unexpected_server_completion(
    completion: std::result::Result<Result<ServerKind>, tokio::task::JoinError>,
) -> Error {
    match completed_server(completion) {
        Ok(kind) => Error::Message(format!("{kind} server stopped unexpectedly")),
        Err(error) => error,
    }
}

fn completed_server(
    completion: std::result::Result<Result<ServerKind>, tokio::task::JoinError>,
) -> Result<ServerKind> {
    completion.map_err(Error::wrap)?
}

#[cfg(test)]
mod tests {
    use super::{AgentRpcSettings, ControllerSettings};
    use loco_rs::{config::Config, environment::Environment};
    use std::path::Path;

    #[test]
    fn controller_rpc_settings_require_an_explicit_mtls_bundle() {
        let settings: ControllerSettings = serde_json::from_value(serde_json::json!({
            "agent_rpc": {
                "enabled": true,
                "binding": "127.0.0.1:50051",
                "cluster_id": "cluster-1",
                "tls": {
                    "certificate_path": "/certs/controller.pem",
                    "private_key_path": "/certs/controller-key.pem",
                    "client_ca_path": "/certs/agent-ca.pem"
                }
            }
        }))
        .unwrap();

        let AgentRpcSettings {
            enabled,
            binding,
            cluster_id,
            tls,
        } = settings.agent_rpc;
        assert!(enabled);
        assert_eq!(binding.to_string(), "127.0.0.1:50051");
        assert_eq!(cluster_id, "cluster-1");
        assert_eq!(tls.client_ca_path.to_string_lossy(), "/certs/agent-ca.pem");
    }

    #[test]
    fn checked_in_non_production_configs_parse() {
        for environment in [Environment::Development, Environment::Test] {
            let config = Config::from_folder(&environment, Path::new("config")).unwrap();
            let settings: ControllerSettings =
                serde_json::from_value(config.settings.unwrap()).unwrap();

            assert!(!settings.agent_rpc.enabled);
        }
    }
}
