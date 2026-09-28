use std::{env, error::Error, net::SocketAddr};

use kontainer_engine::DockerHost;
use kontainer_lifecycle::ShutdownGrace;
use kontainer_rpc_auth::{MutualTlsIdentityFiles, valid_identity_segment};

/// `KONTAINER_ENV`: only development and test may point the agent at a TCP
/// Docker host (ADR 0004).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentEnvironment {
    Production,
    Development,
    Test,
}

impl AgentEnvironment {
    fn parse(value: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        match value {
            "production" => Ok(Self::Production),
            "development" => Ok(Self::Development),
            "test" => Ok(Self::Test),
            _ => Err("KONTAINER_ENV must be production, development, or test".into()),
        }
    }

    const fn allows_tcp_docker_host(self) -> bool {
        matches!(self, Self::Development | Self::Test)
    }
}

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub node_id: String,
    pub cluster_id: String,
    pub binding: SocketAddr,
    pub tls_files: MutualTlsIdentityFiles,
    pub shutdown_grace: ShutdownGrace,
    pub environment: AgentEnvironment,
    pub docker_host: DockerHost,
    /// `https://host:port` of the controller's agent RPC listener.
    pub controller_rpc_endpoint: String,
    /// DNS name the controller certificate must carry.
    pub controller_server_name: String,
    /// `host:port` the controller dials for `NodeAgent`; defaults to the binding.
    pub advertise_address: String,
}

impl AgentConfig {
    pub fn from_environment() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let binding: SocketAddr =
            environment_variable("KONTAINER_NODE_RPC_BINDING", "0.0.0.0:50052").parse()?;
        let config = Self {
            node_id: required_environment_variable("KONTAINER_NODE_ID")?,
            cluster_id: required_environment_variable("KONTAINER_CLUSTER_ID")?,
            binding,
            tls_files: MutualTlsIdentityFiles::new(
                required_environment_variable("KONTAINER_NODE_RPC_TLS_CERTIFICATE_PATH")?,
                required_environment_variable("KONTAINER_NODE_RPC_TLS_PRIVATE_KEY_PATH")?,
                required_environment_variable("KONTAINER_NODE_RPC_TLS_CLIENT_CA_PATH")?,
            ),
            shutdown_grace: shutdown_grace_from_environment_with(|name| env::var(name))?,
            environment: AgentEnvironment::parse(&environment_variable(
                "KONTAINER_ENV",
                "production",
            ))?,
            docker_host: match env::var("KONTAINER_DOCKER_HOST") {
                Ok(value) if !value.trim().is_empty() => value
                    .parse()
                    .map_err(|error| format!("KONTAINER_DOCKER_HOST: {error}"))?,
                _ => DockerHost::default(),
            },
            controller_rpc_endpoint: required_environment_variable(
                "KONTAINER_CONTROLLER_RPC_ENDPOINT",
            )?,
            controller_server_name: required_environment_variable(
                "KONTAINER_CONTROLLER_SERVER_NAME",
            )?,
            advertise_address: match env::var("KONTAINER_NODE_ADVERTISE_ADDRESS") {
                Ok(value) if !value.trim().is_empty() => value.trim().to_owned(),
                _ if binding.ip().is_unspecified() => {
                    return Err("KONTAINER_NODE_ADVERTISE_ADDRESS is required when KONTAINER_NODE_RPC_BINDING listens on every address".into());
                }
                _ => binding.to_string(),
            },
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        if self.docker_host.is_tcp() && !self.environment.allows_tcp_docker_host() {
            return Err(
                "KONTAINER_DOCKER_HOST may select tcp:// only when KONTAINER_ENV is development or test"
                    .into(),
            );
        }
        let Some(authority) = self.controller_rpc_endpoint.strip_prefix("https://") else {
            return Err("KONTAINER_CONTROLLER_RPC_ENDPOINT must be an https:// URL".into());
        };
        if !valid_host_port(authority.trim_end_matches('/')) {
            return Err("KONTAINER_CONTROLLER_RPC_ENDPOINT must be https://host:port".into());
        }
        if !valid_dns_name(&self.controller_server_name) {
            return Err("KONTAINER_CONTROLLER_SERVER_NAME must be a DNS name".into());
        }
        if !valid_host_port(&self.advertise_address) {
            return Err("KONTAINER_NODE_ADVERTISE_ADDRESS must be host:port".into());
        }
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

/// `host:port` with a non-empty host of DNS or IP characters and a non-zero port.
fn valid_host_port(value: &str) -> bool {
    let Some((host, port)) = value.rsplit_once(':') else {
        return false;
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    !host.is_empty()
        && host.len() <= 253
        && host
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".-:".contains(character))
        && port.parse::<u16>().is_ok_and(|port| port > 0)
}

fn valid_dns_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
        })
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
    use super::{AgentConfig, AgentEnvironment, shutdown_grace_from_environment_with};
    use kontainer_engine::DockerHost;
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
            environment: AgentEnvironment::Production,
            docker_host: DockerHost::default(),
            controller_rpc_endpoint: "https://127.0.0.1:50051".to_owned(),
            controller_server_name: "controller.kontainer.internal".to_owned(),
            advertise_address: "127.0.0.1:50052".to_owned(),
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

    #[test]
    fn tcp_docker_host_is_refused_outside_development_and_test() {
        let mut config = AgentConfig {
            node_id: "manager".to_owned(),
            cluster_id: "development".to_owned(),
            binding: "127.0.0.1:50052".parse().unwrap(),
            tls_files: MutualTlsIdentityFiles::new("cert.pem", "key.pem", "ca.pem"),
            shutdown_grace: ShutdownGrace::default(),
            environment: AgentEnvironment::Production,
            docker_host: "tcp://localhost:2375".parse().unwrap(),
            controller_rpc_endpoint: "https://127.0.0.1:50051".to_owned(),
            controller_server_name: "controller.kontainer.internal".to_owned(),
            advertise_address: "127.0.0.1:50052".to_owned(),
        };
        assert_eq!(
            config.validate().unwrap_err().to_string(),
            "KONTAINER_DOCKER_HOST may select tcp:// only when KONTAINER_ENV is development or test"
        );
        for allowed in [AgentEnvironment::Development, AgentEnvironment::Test] {
            config.environment = allowed;
            assert!(config.validate().is_ok());
        }
        config.environment = AgentEnvironment::Production;
        config.docker_host = "unix:///var/run/docker.sock".parse().unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn controller_endpoint_server_name_and_advertise_address_are_validated() {
        let valid = AgentConfig {
            node_id: "manager".to_owned(),
            cluster_id: "development".to_owned(),
            binding: "127.0.0.1:50052".parse().unwrap(),
            tls_files: MutualTlsIdentityFiles::new("cert.pem", "key.pem", "ca.pem"),
            shutdown_grace: ShutdownGrace::default(),
            environment: AgentEnvironment::Production,
            docker_host: DockerHost::default(),
            controller_rpc_endpoint: "https://controller.example:50051".to_owned(),
            controller_server_name: "controller.kontainer.internal".to_owned(),
            advertise_address: "10.0.0.5:50052".to_owned(),
        };
        assert!(valid.validate().is_ok());

        for (endpoint, message) in [
            ("http://127.0.0.1:50051", "must be an https:// URL"),
            ("127.0.0.1:50051", "must be an https:// URL"),
            ("https://127.0.0.1", "must be https://host:port"),
            ("https://:50051", "must be https://host:port"),
            ("https://host:0", "must be https://host:port"),
        ] {
            let mut config = valid.clone();
            config.controller_rpc_endpoint = endpoint.to_owned();
            assert!(
                config.validate().unwrap_err().to_string().contains(message),
                "{endpoint}"
            );
        }
        for name in ["", "under_score.internal", "-leading.internal", "a..b"] {
            let mut config = valid.clone();
            config.controller_server_name = name.to_owned();
            assert!(config.validate().is_err(), "{name}");
        }
        for address in ["", "10.0.0.5", "host:port", "bad host:1"] {
            let mut config = valid.clone();
            config.advertise_address = address.to_owned();
            assert!(config.validate().is_err(), "{address}");
        }
    }

    #[test]
    fn environment_names_are_closed() {
        assert_eq!(
            AgentEnvironment::parse("development").unwrap(),
            AgentEnvironment::Development
        );
        assert_eq!(
            AgentEnvironment::parse("test").unwrap(),
            AgentEnvironment::Test
        );
        assert_eq!(
            AgentEnvironment::parse("production").unwrap(),
            AgentEnvironment::Production
        );
        for invalid in ["", "dev", "Production", "staging"] {
            assert!(AgentEnvironment::parse(invalid).is_err());
        }
    }
}
