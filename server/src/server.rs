use std::{fs, net::SocketAddr, path::PathBuf};

use axum::Router;
use loco_rs::{
    Error, Result,
    app::AppContext,
    boot::{ServeParams, shutdown_signal},
};
use serde::Deserialize;
use tokio::{net::TcpListener, task::JoinSet};
use tokio_util::sync::CancellationToken;
use tonic::transport::{Certificate, Identity, Server, ServerTlsConfig};

use crate::rpc::controller_intake_service;

#[derive(Debug, Clone, Deserialize)]
struct ControllerSettings {
    agent_rpc: AgentRpcSettings,
}

#[derive(Debug, Clone, Deserialize)]
struct AgentRpcSettings {
    enabled: bool,
    binding: SocketAddr,
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
    let cancellation = CancellationToken::new();
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

    if settings.agent_rpc.enabled {
        let rpc_cancellation = cancellation.clone();
        let rpc_settings = settings.agent_rpc;
        servers.spawn(async move {
            Server::builder()
                .tls_config(rpc_tls_config(&rpc_settings.tls)?)
                .map_err(Error::wrap)?
                .add_service(controller_intake_service())
                .serve_with_shutdown(rpc_settings.binding, rpc_cancellation.cancelled_owned())
                .await
                .map_err(Error::wrap)?;

            Ok(ServerKind::AgentRpc)
        });
    }

    let first_completion = tokio::select! {
        _ = shutdown_signal() => None,
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

fn rpc_tls_config(settings: &AgentRpcTlsSettings) -> Result<ServerTlsConfig> {
    let certificate = fs::read(&settings.certificate_path)?;
    let private_key = fs::read(&settings.private_key_path)?;
    let client_ca = fs::read(&settings.client_ca_path)?;

    Ok(ServerTlsConfig::new()
        .identity(Identity::from_pem(certificate, private_key))
        .client_ca_root(Certificate::from_pem(client_ca)))
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
            tls,
        } = settings.agent_rpc;
        assert!(enabled);
        assert_eq!(binding.to_string(), "127.0.0.1:50051");
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
