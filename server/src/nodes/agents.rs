//! Agent clients built per node from membership rows.
//!
//! The endpoint comes from the node's own check-in (`advertise_address`) and the expected mTLS
//! identity from its id, so a request can pick a node but never an address or identity. Clients
//! connect per call; add a channel cache only if a measurement shows it matters.

use std::sync::Arc;

use kontainer_rpc_auth::MutualTlsIdentityFiles;

use crate::rpc::{AgentClient, AgentClientError, NodeAgentClientConfig};

/// DNS names agent certificates carry: `{node_id}.agent.kontainer.internal`.
pub const DEFAULT_AGENT_SERVER_NAME_SUFFIX: &str = kontainer_rpc_auth::AGENT_DNS_SUFFIX;

/// Where to reach one node, as membership recorded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeEndpoint {
    pub node_id: String,
    pub advertise_address: String,
}

/// Builds a client for a node. Production dials over mTLS; tests return fakes.
pub trait AgentDirectory: std::fmt::Debug + Send + Sync {
    fn client(&self, node: &NodeEndpoint) -> Result<Arc<dyn AgentClient>, AgentClientError>;
}

/// Shared directory stored in [`loco_rs::app::AppContext::shared_store`].
#[derive(Debug, Clone)]
pub struct SharedAgentDirectory(pub Arc<dyn AgentDirectory>);

#[derive(Debug, Clone)]
pub struct MtlsAgentDirectory {
    cluster_id: String,
    tls_files: MutualTlsIdentityFiles,
    server_name_suffix: String,
}

impl MtlsAgentDirectory {
    #[must_use]
    pub fn new(
        cluster_id: String,
        tls_files: MutualTlsIdentityFiles,
        server_name_suffix: String,
    ) -> Self {
        Self {
            cluster_id,
            tls_files,
            server_name_suffix,
        }
    }
}

impl AgentDirectory for MtlsAgentDirectory {
    fn client(&self, node: &NodeEndpoint) -> Result<Arc<dyn AgentClient>, AgentClientError> {
        NodeAgentClientConfig::new(
            format!("https://{}", node.advertise_address),
            format!("{}.{}", node.node_id, self.server_name_suffix),
            self.cluster_id.clone(),
            node.node_id.clone(),
            self.tls_files.clone(),
        )
        .map(|client| Arc::new(client) as Arc<dyn AgentClient>)
        .map_err(|_| AgentClientError::Unavailable)
    }
}
