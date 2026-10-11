use std::{env, error::Error, net::SocketAddr};

use kontainer_engine::{ContainerEngine, DockerHost};
use kontainer_lifecycle::ShutdownGrace;
use kontainer_rpc_auth::{
    CONTROLLER_DNS_NAME, ClusterSecret, DEFAULT_CLUSTER_ID, MutualTlsIdentityFiles,
    valid_identity_segment,
};
use tokio_util::sync::CancellationToken;

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
    /// `KONTAINER_NODE_ID`; unset, [`discover_node_id`] asks Docker.
    pub node_id: Option<String>,
    pub cluster_id: String,
    pub binding: SocketAddr,
    pub trust: AgentTrust,
    pub shutdown_grace: ShutdownGrace,
    pub environment: AgentEnvironment,
    pub docker_host: DockerHost,
    /// `https://host:port` of the controller's agent RPC listener.
    pub controller_rpc_endpoint: String,
    /// DNS name the controller certificate must carry.
    pub controller_server_name: String,
    /// `host:port` the controller dials for `NodeAgent`: the setting, else a specific binding,
    /// else unset and [`advertise_toward`] finds it.
    pub advertise_address: Option<String>,
}

/// Where the agent's mTLS identity comes from.
#[derive(Debug, Clone)]
pub enum AgentTrust {
    /// Operator-issued files (`KONTAINER_NODE_RPC_TLS_*`), as `dev/pki.sh` writes them.
    Files(MutualTlsIdentityFiles),
    /// Issued at startup from the shared cluster secret (ADR 0045).
    Secret(ClusterSecret),
}

impl AgentConfig {
    pub fn from_environment() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let binding: SocketAddr =
            environment_variable("KONTAINER_NODE_RPC_BINDING", "0.0.0.0:50052").parse()?;
        let tls_paths = [
            "KONTAINER_NODE_RPC_TLS_CERTIFICATE_PATH",
            "KONTAINER_NODE_RPC_TLS_PRIVATE_KEY_PATH",
            "KONTAINER_NODE_RPC_TLS_CLIENT_CA_PATH",
        ]
        .map(optional_environment_variable);
        let trust = match tls_paths {
            [Some(certificate), Some(private_key), Some(client_ca)] => AgentTrust::Files(
                MutualTlsIdentityFiles::new(certificate, private_key, client_ca),
            ),
            [None, None, None] => AgentTrust::Secret(ClusterSecret::from_environment()?.ok_or(
                "set KONTAINER_SECRET (or KONTAINER_SECRET_FILE) to the controller's value",
            )?),
            _ => return Err("set all three KONTAINER_NODE_RPC_TLS_* paths or none".into()),
        };
        let config = Self {
            node_id: optional_environment_variable("KONTAINER_NODE_ID"),
            cluster_id: environment_variable("KONTAINER_CLUSTER_ID", DEFAULT_CLUSTER_ID),
            binding,
            trust,
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
            controller_rpc_endpoint: environment_variable(
                "KONTAINER_CONTROLLER_RPC_ENDPOINT",
                "https://controller:50051",
            ),
            controller_server_name: environment_variable(
                "KONTAINER_CONTROLLER_SERVER_NAME",
                CONTROLLER_DNS_NAME,
            ),
            advertise_address: optional_environment_variable("KONTAINER_NODE_ADVERTISE_ADDRESS")
                .or_else(|| (!binding.ip().is_unspecified()).then(|| binding.to_string())),
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        if self.docker_host.is_remote_tcp() {
            return Err(
                "KONTAINER_DOCKER_HOST may select tcp:// only for a loopback host (localhost, 127.0.0.0/8, ::1)"
                    .into(),
            );
        }
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
        if let Some(address) = &self.advertise_address
            && !valid_host_port(address)
        {
            return Err("KONTAINER_NODE_ADVERTISE_ADDRESS must be host:port".into());
        }
        if let Some(node_id) = &self.node_id
            && !valid_identity_segment(node_id)
        {
            return Err("KONTAINER_NODE_ID must be a canonical workload identity segment".into());
        }
        if !valid_identity_segment(&self.cluster_id) {
            return Err(
                "KONTAINER_CLUSTER_ID must be a canonical workload identity segment".into(),
            );
        }
        Ok(())
    }
}

/// The node's Swarm node id, or its hostname outside a Swarm. Docker must answer.
pub async fn discover_node_id(
    engine: &dyn ContainerEngine,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let info = engine
        .node_info(&CancellationToken::new())
        .await
        .map_err(|_| "Docker did not answer, so the node id is unknown; set KONTAINER_NODE_ID")?;
    let node_id = info.swarm_node_id.unwrap_or(info.hostname);
    if !valid_identity_segment(&node_id) {
        return Err(
            "this node's Swarm id or hostname is not a valid node id; set KONTAINER_NODE_ID".into(),
        );
    }
    Ok(node_id)
}

/// `ip:port` of this host on the route to the controller, which is the address the controller can
/// dial back. Connecting a UDP socket only picks the route; nothing is sent.
pub async fn advertise_toward(
    controller_rpc_endpoint: &str,
    port: u16,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let authority = controller_rpc_endpoint
        .strip_prefix("https://")
        .unwrap_or(controller_rpc_endpoint)
        .trim_end_matches('/');
    let controller = tokio::net::lookup_host(authority)
        .await?
        .next()
        .ok_or("the controller's address did not resolve")?;
    let probe = std::net::UdpSocket::bind(if controller.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    })?;
    probe.connect(controller)?;
    Ok(SocketAddr::new(probe.local_addr()?.ip(), port).to_string())
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

fn optional_environment_variable(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn environment_variable(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        AgentConfig, AgentEnvironment, AgentTrust, advertise_toward, discover_node_id,
        shutdown_grace_from_environment_with,
    };
    use async_trait::async_trait;
    use kontainer_engine::{ContainerEngine, DockerHost, EngineError, NodeInfo};
    use kontainer_lifecycle::ShutdownGrace;
    use kontainer_rpc_auth::MutualTlsIdentityFiles;
    use tokio_util::sync::CancellationToken;

    fn files() -> AgentTrust {
        AgentTrust::Files(MutualTlsIdentityFiles::new("cert.pem", "key.pem", "ca.pem"))
    }

    struct NodeInfoEngine(NodeInfo);

    #[async_trait]
    impl ContainerEngine for NodeInfoEngine {
        async fn is_available(&self) -> Result<bool, EngineError> {
            Ok(true)
        }

        async fn node_info(&self, _: &CancellationToken) -> Result<NodeInfo, EngineError> {
            Ok(self.0.clone())
        }
    }

    #[tokio::test]
    async fn the_node_id_is_the_swarm_id_then_the_hostname() {
        let node = |swarm_node_id: Option<&str>, hostname: &str| {
            NodeInfoEngine(NodeInfo {
                hostname: hostname.to_owned(),
                swarm_node_id: swarm_node_id.map(str::to_owned),
                ..NodeInfo::default()
            })
        };
        assert_eq!(
            discover_node_id(&node(Some("tj42rq5nzwf8"), "vm-1"))
                .await
                .unwrap(),
            "tj42rq5nzwf8"
        );
        assert_eq!(discover_node_id(&node(None, "vm-1")).await.unwrap(), "vm-1");
        let error = discover_node_id(&node(None, "bad/host")).await.unwrap_err();
        assert!(error.to_string().contains("set KONTAINER_NODE_ID"));

        // The default engine method answers UnsupportedCapability, like a daemon that is down.
        struct SilentEngine;
        #[async_trait]
        impl ContainerEngine for SilentEngine {
            async fn is_available(&self) -> Result<bool, EngineError> {
                Ok(false)
            }
        }
        let error = discover_node_id(&SilentEngine).await.unwrap_err();
        assert!(error.to_string().contains("set KONTAINER_NODE_ID"));
    }

    #[tokio::test]
    async fn the_advertise_address_is_this_host_on_the_route_to_the_controller() {
        assert_eq!(
            advertise_toward("https://127.0.0.1:50051", 50052)
                .await
                .unwrap(),
            "127.0.0.1:50052"
        );
        assert_eq!(
            advertise_toward("https://127.0.0.1:50051/", 7)
                .await
                .unwrap(),
            "127.0.0.1:7"
        );
    }

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
            node_id: Some("node-1".to_owned()),
            cluster_id: "cluster-1".to_owned(),
            binding: "127.0.0.1:50052".parse().unwrap(),
            trust: files(),
            shutdown_grace: ShutdownGrace::default(),
            environment: AgentEnvironment::Production,
            docker_host: DockerHost::default(),
            controller_rpc_endpoint: "https://127.0.0.1:50051".to_owned(),
            controller_server_name: "controller.kontainer.internal".to_owned(),
            advertise_address: Some("127.0.0.1:50052".to_owned()),
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
            config.node_id = Some(invalid.to_owned());
            config.cluster_id = "cluster-1".to_owned();
            let error = config.validate().unwrap_err().to_string();
            assert_eq!(
                error,
                "KONTAINER_NODE_ID must be a canonical workload identity segment"
            );

            config.node_id = Some("node-1".to_owned());
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
            node_id: Some("manager".to_owned()),
            cluster_id: "development".to_owned(),
            binding: "127.0.0.1:50052".parse().unwrap(),
            trust: files(),
            shutdown_grace: ShutdownGrace::default(),
            environment: AgentEnvironment::Production,
            docker_host: "tcp://localhost:2375".parse().unwrap(),
            controller_rpc_endpoint: "https://127.0.0.1:50051".to_owned(),
            controller_server_name: "controller.kontainer.internal".to_owned(),
            advertise_address: Some("127.0.0.1:50052".to_owned()),
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

        config.docker_host = "tcp://192.168.1.20:2375".parse().unwrap();
        for environment in [AgentEnvironment::Development, AgentEnvironment::Test] {
            config.environment = environment;
            assert_eq!(
                config.validate().unwrap_err().to_string(),
                "KONTAINER_DOCKER_HOST may select tcp:// only for a loopback host (localhost, 127.0.0.0/8, ::1)"
            );
        }
    }

    #[test]
    fn controller_endpoint_server_name_and_advertise_address_are_validated() {
        let valid = AgentConfig {
            node_id: Some("manager".to_owned()),
            cluster_id: "development".to_owned(),
            binding: "127.0.0.1:50052".parse().unwrap(),
            trust: files(),
            shutdown_grace: ShutdownGrace::default(),
            environment: AgentEnvironment::Production,
            docker_host: DockerHost::default(),
            controller_rpc_endpoint: "https://controller.example:50051".to_owned(),
            controller_server_name: "controller.kontainer.internal".to_owned(),
            advertise_address: Some("10.0.0.5:50052".to_owned()),
        };
        assert!(valid.validate().is_ok());
        let mut unresolved = valid.clone();
        unresolved.node_id = None;
        unresolved.advertise_address = None;
        assert!(unresolved.validate().is_ok(), "both are discovered later");

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
            config.advertise_address = Some(address.to_owned());
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
