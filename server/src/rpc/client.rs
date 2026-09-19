#![forbid(unsafe_code)]

//! Production controller-to-agent client for one statically configured node.
//!
//! This is the only production path that dials a node agent. It calls
//! `Describe` with explicit connect and RPC deadlines, verifies the peer
//! workload identity during TLS, cross-checks the reported node ID, and
//! propagates caller and shutdown cancellation without spawning detached
//! work. There is no discovery, retry, pooling, or channel management here;
//! later membership work owns those concerns.

use std::time::Duration;

use kontainer_agent_proto::v1::{DescribeRequest, node_agent_client::NodeAgentClient};
use kontainer_rpc_auth::{MutualTlsIdentityFiles, WorkloadIdentity};
use tokio_util::sync::CancellationToken;
use tonic::transport::{Channel, Endpoint};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const RPC_TIMEOUT: Duration = Duration::from_secs(2);

/// Validated static configuration for the single agent endpoint.
#[derive(Debug, Clone)]
pub struct NodeAgentClientConfig {
    endpoint: String,
    server_name: String,
    expected: WorkloadIdentity,
    tls_files: MutualTlsIdentityFiles,
    connect_timeout: Duration,
    rpc_timeout: Duration,
}

impl NodeAgentClientConfig {
    /// Validate static settings and TLS material without connecting.
    ///
    /// Network unavailability must not fail here; only malformed settings
    /// and unreadable or unparsable TLS files fail. Building the Tonic
    /// endpoint parses the certificate material, so mismatched keys fail
    /// here rather than on the first call.
    pub fn new(
        endpoint: String,
        server_name: String,
        cluster_id: String,
        node_id: String,
        tls_files: MutualTlsIdentityFiles,
    ) -> Result<Self, AgentClientConfigError> {
        Self::with_timeouts(
            endpoint,
            server_name,
            cluster_id,
            node_id,
            tls_files,
            CONNECT_TIMEOUT,
            RPC_TIMEOUT,
        )
    }

    fn with_timeouts(
        endpoint: String,
        server_name: String,
        cluster_id: String,
        node_id: String,
        tls_files: MutualTlsIdentityFiles,
        connect_timeout: Duration,
        rpc_timeout: Duration,
    ) -> Result<Self, AgentClientConfigError> {
        validate_endpoint(&endpoint)?;
        validate_server_name(&server_name)?;
        validate_identity_segment(&cluster_id, "cluster ID")?;
        validate_identity_segment(&node_id, "node ID")?;

        let expected = WorkloadIdentity::NodeAgent {
            cluster_id,
            node_id,
        };

        let tls_config = tls_files
            .client_config_for_custom_verifier(server_name.clone())
            .map_err(|error| {
                AgentClientConfigError::InvalidTls(format!(
                    "agent client TLS files are invalid: {error}"
                ))
            })?;
        let verifier = tls_files
            .outbound_verifier(expected.clone())
            .map_err(|error| {
                AgentClientConfigError::InvalidTls(format!(
                    "agent client TLS files are invalid: {error}"
                ))
            })?;

        // Eagerly parse certificate material. This does not connect; an
        // unreachable agent still constructs successfully and fails only
        // when `describe` is called.
        Endpoint::from_shared(endpoint.clone())
            .map_err(|_| AgentClientConfigError::InvalidEndpoint)?
            .connect_timeout(connect_timeout)
            .timeout(rpc_timeout)
            .tls_config_with_verifier(tls_config, verifier)
            .map_err(|_| {
                AgentClientConfigError::InvalidTls("agent client TLS files are invalid".to_owned())
            })?;

        Ok(Self {
            endpoint,
            server_name,
            expected,
            tls_files,
            connect_timeout,
            rpc_timeout,
        })
    }

    fn endpoint_prototype(&self) -> Result<Endpoint, AgentClientError> {
        let tls_config = self
            .tls_files
            .client_config_for_custom_verifier(self.server_name.clone())
            .map_err(|_| AgentClientError::Unavailable)?;
        let verifier = self
            .tls_files
            .outbound_verifier(self.expected.clone())
            .map_err(|_| AgentClientError::Unavailable)?;
        Endpoint::from_shared(self.endpoint.clone())
            .map_err(|_| AgentClientError::Unavailable)?
            .connect_timeout(self.connect_timeout)
            .timeout(self.rpc_timeout)
            .tls_config_with_verifier(tls_config, verifier)
            .map_err(|_| AgentClientError::Unavailable)
    }

    /// Call `Describe` on the statically configured agent.
    ///
    /// The endpoint, DNS name, and expected identity come only from this
    /// client's configuration. No request input selects or overrides them.
    pub async fn describe(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<AgentDescription, AgentClientError> {
        let channel = self.channel(cancellation).await?;

        let mut client = NodeAgentClient::new(channel);
        let mut request = tonic::Request::new(DescribeRequest {});
        request.set_timeout(self.rpc_timeout);

        let response = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(self.rpc_timeout, client.describe(request)) => {
                match result {
                    Ok(Ok(response)) => response.into_inner(),
                    Ok(Err(status)) => return Err(map_rpc_status(status)),
                    Err(_) => return Err(AgentClientError::TimedOut),
                }
            }
        };

        let expected_node = self
            .expected
            .node_id()
            .ok_or(AgentClientError::InvalidResponse)?;
        if response.node_id != expected_node {
            return Err(AgentClientError::IdentityMismatch);
        }

        Ok(AgentDescription {
            node_id: response.node_id,
            agent_version: response.agent_version,
            architecture: response.architecture,
            docker_connected: response.docker_connected,
        })
    }

    /// Open the single static channel, observing connect deadlines and
    /// caller/shutdown cancellation without spawning detached work.
    async fn channel(&self, cancellation: &CancellationToken) -> Result<Channel, AgentClientError> {
        let endpoint = self.endpoint_prototype()?;

        tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(AgentClientError::Cancelled),
            result = tokio::time::timeout(self.connect_timeout, endpoint.connect()) => {
                match result {
                    Ok(Ok(channel)) => Ok(channel),
                    Ok(Err(_)) => Err(AgentClientError::Unavailable),
                    Err(_) => Err(AgentClientError::TimedOut),
                }
            }
        }
    }
}

/// Controller-owned `Describe` result, distinct from protobuf types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDescription {
    pub node_id: String,
    pub agent_version: String,
    pub architecture: String,
    pub docker_connected: bool,
}

/// Stable, redacted call failures. No endpoint, address, TLS, or daemon
/// detail crosses this boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentClientError {
    /// The agent could not be reached or its certificate was rejected.
    Unavailable,
    /// The connect or RPC deadline expired.
    TimedOut,
    /// The caller or controller shutdown cancelled the call.
    Cancelled,
    /// The peer identity or reported node ID did not match configuration.
    IdentityMismatch,
    /// The response was malformed.
    InvalidResponse,
}

impl std::fmt::Display for AgentClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("agent is unavailable"),
            Self::TimedOut => formatter.write_str("agent request timed out"),
            Self::Cancelled => formatter.write_str("agent request was cancelled"),
            Self::IdentityMismatch => formatter.write_str("agent identity did not match"),
            Self::InvalidResponse => formatter.write_str("agent response was invalid"),
        }
    }
}

impl std::error::Error for AgentClientError {}

/// Startup validation failures. Messages stay generic and never include
/// certificate material or secret values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentClientConfigError {
    InvalidEndpoint,
    InvalidServerName,
    InvalidIdentity(&'static str),
    InvalidTls(String),
}

impl std::fmt::Display for AgentClientConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidEndpoint => formatter.write_str("agent endpoint is invalid"),
            Self::InvalidServerName => formatter.write_str("agent server name is invalid"),
            Self::InvalidIdentity(which) => {
                write!(formatter, "agent {which} is invalid")
            }
            Self::InvalidTls(detail) => formatter.write_str(detail),
        }
    }
}

impl std::error::Error for AgentClientConfigError {}

fn validate_endpoint(endpoint: &str) -> Result<(), AgentClientConfigError> {
    let parsed = url::Url::parse(endpoint).map_err(|_| AgentClientConfigError::InvalidEndpoint)?;
    if parsed.scheme() != "https" {
        return Err(AgentClientConfigError::InvalidEndpoint);
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AgentClientConfigError::InvalidEndpoint);
    }
    if parsed.host_str().is_none() {
        return Err(AgentClientConfigError::InvalidEndpoint);
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(AgentClientConfigError::InvalidEndpoint);
    }
    // A routing path would allow one URL to address different agents;
    // the static endpoint must name exactly one agent origin.
    if parsed.path() != "/" && !parsed.path().is_empty() {
        return Err(AgentClientConfigError::InvalidEndpoint);
    }
    Ok(())
}

fn validate_server_name(server_name: &str) -> Result<(), AgentClientConfigError> {
    if server_name.is_empty() || server_name.len() > 253 {
        return Err(AgentClientConfigError::InvalidServerName);
    }
    if !server_name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
    {
        return Err(AgentClientConfigError::InvalidServerName);
    }
    if server_name.starts_with('-')
        || server_name.starts_with('.')
        || server_name.ends_with('-')
        || server_name.ends_with('.')
        || server_name.contains("..")
    {
        return Err(AgentClientConfigError::InvalidServerName);
    }
    Ok(())
}

fn validate_identity_segment(
    value: &str,
    which: &'static str,
) -> Result<(), AgentClientConfigError> {
    if !kontainer_rpc_auth::valid_identity_segment(value) {
        return Err(AgentClientConfigError::InvalidIdentity(which));
    }
    Ok(())
}

fn map_rpc_status(status: tonic::Status) -> AgentClientError {
    use tonic::Code;
    match status.code() {
        Code::Cancelled => AgentClientError::Cancelled,
        Code::DeadlineExceeded => AgentClientError::TimedOut,
        Code::Unauthenticated | Code::PermissionDenied => AgentClientError::IdentityMismatch,
        Code::Unavailable => AgentClientError::Unavailable,
        _ => AgentClientError::InvalidResponse,
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentClientConfigError, validate_endpoint, validate_server_name};

    #[test]
    fn endpoint_rejects_non_static_origins() {
        for endpoint in [
            "",
            "http://127.0.0.1:50052",
            "https://127.0.0.1:50052/containers",
            "https://127.0.0.1:50052/?cursor=1",
            "https://127.0.0.1:50052#fragment",
            "https://user:pass@127.0.0.1:50052/",
            "https://[::1",
            "not-a-url",
        ] {
            assert_eq!(
                validate_endpoint(endpoint),
                Err(AgentClientConfigError::InvalidEndpoint),
                "{endpoint}"
            );
        }
        assert!(validate_endpoint("https://127.0.0.1:50052").is_ok());
        assert!(validate_endpoint("https://127.0.0.1:50052/").is_ok());
    }

    #[test]
    fn server_names_reject_empty_and_malformed_hosts() {
        for name in [
            "",
            "-agent.test",
            ".agent.test",
            "agent..test",
            "agent_test",
            "agent test",
        ] {
            assert_eq!(
                validate_server_name(name),
                Err(AgentClientConfigError::InvalidServerName),
                "{name}"
            );
        }
        assert!(validate_server_name("node-1.agent.kontainer.test").is_ok());
    }

    #[test]
    fn error_messages_stay_generic() {
        assert_eq!(
            AgentClientConfigError::InvalidEndpoint.to_string(),
            "agent endpoint is invalid"
        );
        assert_eq!(
            super::AgentClientError::Unavailable.to_string(),
            "agent is unavailable"
        );
        assert_eq!(
            super::AgentClientError::IdentityMismatch.to_string(),
            "agent identity did not match"
        );
        assert_eq!(
            super::map_rpc_status(tonic::Status::deadline_exceeded("slow")),
            super::AgentClientError::TimedOut
        );
        assert_eq!(
            super::map_rpc_status(tonic::Status::unavailable("down")),
            super::AgentClientError::Unavailable
        );
    }
}
