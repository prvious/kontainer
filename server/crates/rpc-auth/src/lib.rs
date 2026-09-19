#![forbid(unsafe_code)]

mod identity;
mod interceptor;
mod tls;
mod verifier;

pub use identity::{
    AuthenticatedPeerIdentity, IdentityError, WorkloadIdentity, WorkloadKind,
    valid_identity_segment,
};
pub use interceptor::PeerIdentityInterceptor;
pub use tls::MutualTlsIdentityFiles;
pub use verifier::{OutboundPeerVerifier, VerifierBuildError};
