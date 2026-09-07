use std::{env, error::Error, net::SocketAddr};

use kontainer_rpc_auth::MutualTlsIdentityFiles;

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub node_id: String,
    pub cluster_id: String,
    pub binding: SocketAddr,
    pub tls_files: MutualTlsIdentityFiles,
}

impl AgentConfig {
    pub fn from_environment() -> Result<Self, Box<dyn Error + Send + Sync>> {
        Ok(Self {
            node_id: required_environment_variable("KONTAINER_NODE_ID")?,
            cluster_id: required_environment_variable("KONTAINER_CLUSTER_ID")?,
            binding: environment_variable("KONTAINER_NODE_RPC_BINDING", "0.0.0.0:50052").parse()?,
            tls_files: MutualTlsIdentityFiles::new(
                required_environment_variable("KONTAINER_NODE_RPC_TLS_CERTIFICATE_PATH")?,
                required_environment_variable("KONTAINER_NODE_RPC_TLS_PRIVATE_KEY_PATH")?,
                required_environment_variable("KONTAINER_NODE_RPC_TLS_CLIENT_CA_PATH")?,
            ),
        })
    }
}

fn required_environment_variable(name: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("required environment variable {name} is not set").into())
}

fn environment_variable(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}
