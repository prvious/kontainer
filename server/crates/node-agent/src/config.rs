use std::{env, error::Error, net::SocketAddr};

use kontainer_lifecycle::ShutdownGrace;
use kontainer_rpc_auth::{MutualTlsIdentityFiles, valid_identity_segment};

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub node_id: String,
    pub cluster_id: String,
    pub binding: SocketAddr,
    pub tls_files: MutualTlsIdentityFiles,
    pub shutdown_grace: ShutdownGrace,
}

impl AgentConfig {
    pub fn from_environment() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let config = Self {
            node_id: required_environment_variable("KONTAINER_NODE_ID")?,
            cluster_id: required_environment_variable("KONTAINER_CLUSTER_ID")?,
            binding: environment_variable("KONTAINER_NODE_RPC_BINDING", "0.0.0.0:50052").parse()?,
            tls_files: MutualTlsIdentityFiles::new(
                required_environment_variable("KONTAINER_NODE_RPC_TLS_CERTIFICATE_PATH")?,
                required_environment_variable("KONTAINER_NODE_RPC_TLS_PRIVATE_KEY_PATH")?,
                required_environment_variable("KONTAINER_NODE_RPC_TLS_CLIENT_CA_PATH")?,
            ),
            shutdown_grace: shutdown_grace_from_environment_with(|name| env::var(name))?,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        for (name, value) in [
            ("KONTAINER_NODE_ID", self.node_id.as_str()),
            ("KONTAINER_CLUSTER_ID", self.cluster_id.as_str()),
        ] {
            if !valid_identity_segment(value) {
                return Err(format!("{name} must be a canonical workload identity segment").into());
            }
        }
        Ok(())
    }
}

fn shutdown_grace_from_environment_with(
    lookup: impl FnOnce(&str) -> Result<String, env::VarError>,
) -> Result<ShutdownGrace, Box<dyn Error + Send + Sync>> {
    match lookup("KONTAINER_NODE_SHUTDOWN_GRACE") {
        Ok(value) => Ok(value.parse()?),
        Err(env::VarError::NotPresent) => Ok(ShutdownGrace::default()),
        Err(env::VarError::NotUnicode(_)) => {
            Err("KONTAINER_NODE_SHUTDOWN_GRACE is not valid text".into())
        }
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

#[cfg(test)]
mod tests {
    use super::{AgentConfig, shutdown_grace_from_environment_with};
    use kontainer_lifecycle::ShutdownGrace;
    use kontainer_rpc_auth::MutualTlsIdentityFiles;

    #[test]
    fn shutdown_grace_environment_values_use_the_shared_parser_and_default() {
        assert_eq!(
            shutdown_grace_from_environment_with(|name| {
                assert_eq!(name, "KONTAINER_NODE_SHUTDOWN_GRACE");
                Err(std::env::VarError::NotPresent)
            })
            .unwrap()
            .duration(),
            std::time::Duration::from_secs(20),
        );
        assert_eq!(
            shutdown_grace_from_environment_with(|name| {
                assert_eq!(name, "KONTAINER_NODE_SHUTDOWN_GRACE");
                Ok("750ms".to_owned())
            })
            .unwrap()
            .duration(),
            std::time::Duration::from_millis(750),
        );
        for invalid in ["0s", "1h"] {
            assert!(
                shutdown_grace_from_environment_with(|name| {
                    assert_eq!(name, "KONTAINER_NODE_SHUTDOWN_GRACE");
                    Ok(invalid.to_owned())
                })
                .is_err()
            );
        }
    }

    #[test]
    fn node_and_cluster_settings_use_the_certificate_identity_rules() {
        let mut config = AgentConfig {
            node_id: "node-1".to_owned(),
            cluster_id: "cluster-1".to_owned(),
            binding: "127.0.0.1:50052".parse().unwrap(),
            tls_files: MutualTlsIdentityFiles::new("cert.pem", "key.pem", "ca.pem"),
            shutdown_grace: ShutdownGrace::default(),
        };
        assert!(config.validate().is_ok());

        for invalid in [
            "",
            ".",
            "..",
            "node/other",
            " secret-value ",
            "node%2fother",
        ] {
            config.node_id = invalid.to_owned();
            config.cluster_id = "cluster-1".to_owned();
            let error = config.validate().unwrap_err().to_string();
            assert_eq!(
                error,
                "KONTAINER_NODE_ID must be a canonical workload identity segment"
            );

            config.node_id = "node-1".to_owned();
            config.cluster_id = invalid.to_owned();
            let error = config.validate().unwrap_err().to_string();
            assert_eq!(
                error,
                "KONTAINER_CLUSTER_ID must be a canonical workload identity segment"
            );
        }
    }
}
