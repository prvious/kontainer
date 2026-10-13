#![forbid(unsafe_code)]

mod cluster_secret;
mod identity;
mod interceptor;
mod server;
mod tls;
mod verifier;

pub use cluster_secret::{
    AGENT_DNS_SUFFIX, CONTROLLER_DNS_NAME, ClusterSecret, DEFAULT_CLUSTER_ID,
    MINIMUM_SECRET_LENGTH, runtime_directory,
};
pub use identity::{
    AuthenticatedPeerIdentity, IdentityError, WorkloadIdentity, WorkloadKind,
    valid_identity_segment,
};
pub use interceptor::PeerIdentityInterceptor;
pub use server::{serve_rpc_connection, serve_rpc_listener, spawn_tracked};
pub use tls::MutualTlsIdentityFiles;
pub use verifier::{OutboundPeerVerifier, VerifierBuildError};
