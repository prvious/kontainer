#![forbid(unsafe_code)]

mod identity;
mod interceptor;
mod tls;

pub use identity::{
    AuthenticatedPeerIdentity, IdentityError, WorkloadIdentity, WorkloadKind,
    valid_identity_segment,
};
pub use interceptor::PeerIdentityInterceptor;
pub use tls::MutualTlsIdentityFiles;
