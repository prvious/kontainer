use tonic::{Request, Status, service::Interceptor};

use crate::{
    AuthenticatedPeerIdentity, IdentityError, WorkloadIdentity, WorkloadKind,
    valid_identity_segment,
};

#[derive(Debug, Clone)]
pub struct PeerIdentityInterceptor {
    cluster_id: String,
    required_kind: WorkloadKind,
}

impl PeerIdentityInterceptor {
    pub fn new(
        cluster_id: impl Into<String>,
        required_kind: WorkloadKind,
    ) -> Result<Self, IdentityError> {
        let cluster_id = cluster_id.into();
        if !valid_identity_segment(&cluster_id) {
            return Err(IdentityError::InvalidWorkloadIdentity);
        }
        Ok(Self {
            cluster_id,
            required_kind,
        })
    }
}

impl Interceptor for PeerIdentityInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let certificates = request
            .peer_certs()
            .ok_or_else(|| Status::unauthenticated("a verified peer certificate is required"))?;
        let leaf = certificates
            .first()
            .ok_or_else(|| Status::unauthenticated("a verified peer certificate is required"))?;
        let identity = WorkloadIdentity::from_certificate_der(leaf.as_ref())
            .map_err(|_| Status::unauthenticated("peer workload identity is invalid"))?;

        if identity.cluster_id() != self.cluster_id || identity.kind() != self.required_kind {
            return Err(Status::unauthenticated(
                "peer workload identity is not allowed for this service",
            ));
        }

        request
            .extensions_mut()
            .insert(AuthenticatedPeerIdentity::new(identity));
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use super::{IdentityError, PeerIdentityInterceptor, WorkloadKind};

    #[test]
    fn invalid_configured_clusters_fail_before_peer_admission() {
        for invalid in ["", ".", "..", "cluster/other", " secret-value "] {
            assert_eq!(
                PeerIdentityInterceptor::new(invalid, WorkloadKind::NodeAgent).unwrap_err(),
                IdentityError::InvalidWorkloadIdentity,
            );
        }
        assert!(PeerIdentityInterceptor::new("cluster-1", WorkloadKind::Controller).is_ok());
    }
}
