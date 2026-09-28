#![forbid(unsafe_code)]

//! Outbound mTLS verification for controller-initiated RPC.
//!
//! The standard WebPKI verifier still owns CA, DNS-name, validity, and EKU
//! checks. This wrapper adds the Kontainer requirement that the peer present
//! exactly one URI SAN matching the statically configured workload identity.
//! It is generic over [`WorkloadIdentity`] so the node-agent check-in client
//! (#95) can reuse the same seam for controller identities.

use std::{fmt, sync::Arc};

use rustls::{
    DigitallySignedStruct, DistinguishedName, Error as RustlsError, RootCertStore, SignatureScheme,
    client::{
        WebPkiServerVerifier,
        danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    },
    crypto::{CryptoProvider, ring::default_provider},
    pki_types::{CertificateDer, ServerName, UnixTime, pem::PemObject},
};

use crate::{WorkloadIdentity, valid_identity_segment};

/// Verifier that delegates standard checks to WebPKI, then enforces the
/// expected Kontainer workload identity from the leaf URI SAN.
pub struct OutboundPeerVerifier {
    inner: Arc<WebPkiServerVerifier>,
    expected: WorkloadIdentity,
    /// Accept any controller instance of the expected cluster. Agents dialing the controller
    /// know its cluster but not which replica answers; the kind and cluster still must match.
    any_controller_instance: bool,
}

impl OutboundPeerVerifier {
    /// Build a verifier for `expected` using `peer_ca_pem` as the only trust
    /// root. Fails when the CA bundle is missing, malformed, or empty, and
    /// when the expected identity contains an invalid segment.
    pub fn new(peer_ca_pem: &[u8], expected: WorkloadIdentity) -> Result<Self, VerifierBuildError> {
        Self::build(peer_ca_pem, expected, false)
    }

    /// Build a verifier that accepts any controller instance of `cluster_id`.
    pub fn for_controller_of(
        peer_ca_pem: &[u8],
        cluster_id: impl Into<String>,
    ) -> Result<Self, VerifierBuildError> {
        let expected = WorkloadIdentity::Controller {
            cluster_id: cluster_id.into(),
            instance_id: String::new(),
        };
        Self::build(peer_ca_pem, expected, true)
    }

    fn build(
        peer_ca_pem: &[u8],
        expected: WorkloadIdentity,
        any_controller_instance: bool,
    ) -> Result<Self, VerifierBuildError> {
        if !valid_identity_segment(expected.cluster_id()) {
            return Err(VerifierBuildError::InvalidIdentity);
        }
        if let Some(node_id) = expected.node_id()
            && !valid_identity_segment(node_id)
        {
            return Err(VerifierBuildError::InvalidIdentity);
        }

        let mut roots = RootCertStore::empty();
        let mut parsed = 0;
        for result in CertificateDer::pem_slice_iter(peer_ca_pem) {
            let cert = result.map_err(|_| VerifierBuildError::InvalidCa)?;
            roots.add(cert).map_err(|_| VerifierBuildError::InvalidCa)?;
            parsed += 1;
        }
        if parsed == 0 {
            return Err(VerifierBuildError::InvalidCa);
        }

        let provider: Arc<CryptoProvider> = Arc::new(default_provider());
        let inner = WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider)
            .build()
            .map_err(|_| VerifierBuildError::InvalidCa)?;

        Ok(Self {
            inner,
            expected,
            any_controller_instance,
        })
    }

    fn accepts(&self, actual: &WorkloadIdentity) -> bool {
        identity_accepted(&self.expected, self.any_controller_instance, actual)
    }

    /// Expected workload identity, for tests and diagnostics that must not
    /// log certificate material.
    #[must_use]
    pub fn expected(&self) -> &WorkloadIdentity {
        &self.expected
    }
}

impl fmt::Debug for OutboundPeerVerifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Cluster and node IDs aid routing but must not leak through debug
        // output owned by this verifier; the kind is enough to distinguish
        // controller and agent expectations.
        formatter
            .debug_struct("OutboundPeerVerifier")
            .field("expected_kind", &self.expected.kind())
            .finish_non_exhaustive()
    }
}

impl ServerCertVerifier for OutboundPeerVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        // Standard CA, DNS-name, validity, and EKU verification first. Any
        // failure here fails the handshake before identity comparison.
        self.inner.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        )?;

        let actual = WorkloadIdentity::from_certificate_der(end_entity.as_ref()).map_err(|_| {
            RustlsError::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            )
        })?;

        if !self.accepts(&actual) {
            return Err(RustlsError::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            ));
        }

        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }

    fn root_hint_subjects(&self) -> Option<&[DistinguishedName]> {
        self.inner.root_hint_subjects()
    }
}

fn identity_accepted(
    expected: &WorkloadIdentity,
    any_controller_instance: bool,
    actual: &WorkloadIdentity,
) -> bool {
    if any_controller_instance {
        matches!(actual, WorkloadIdentity::Controller { cluster_id, .. }
            if cluster_id == expected.cluster_id())
    } else {
        actual == expected
    }
}

/// Fail-closed construction errors. Messages stay generic; file paths are
/// added by the caller that owns the settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifierBuildError {
    /// The CA bundle could not be parsed or contained no certificates.
    InvalidCa,
    /// The expected cluster or node ID is not a canonical segment.
    InvalidIdentity,
}

impl fmt::Display for VerifierBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCa => formatter.write_str("peer CA bundle is invalid"),
            Self::InvalidIdentity => formatter.write_str("expected peer identity is invalid"),
        }
    }
}

impl std::error::Error for VerifierBuildError {}

#[cfg(test)]
mod tests {
    use super::{OutboundPeerVerifier, VerifierBuildError, WorkloadIdentity, identity_accepted};

    fn agent_identity() -> WorkloadIdentity {
        WorkloadIdentity::NodeAgent {
            cluster_id: "cluster-1".to_owned(),
            node_id: "node-1".to_owned(),
        }
    }

    #[test]
    fn invalid_identity_segments_fail_before_ca_parsing() {
        let invalid = WorkloadIdentity::NodeAgent {
            cluster_id: "cluster/other".to_owned(),
            node_id: "node-1".to_owned(),
        };
        assert_eq!(
            OutboundPeerVerifier::new(b"not-a-ca", invalid).unwrap_err(),
            VerifierBuildError::InvalidIdentity
        );

        let invalid_node = WorkloadIdentity::NodeAgent {
            cluster_id: "cluster-1".to_owned(),
            node_id: "..".to_owned(),
        };
        assert_eq!(
            OutboundPeerVerifier::new(b"not-a-ca", invalid_node).unwrap_err(),
            VerifierBuildError::InvalidIdentity
        );
    }

    #[test]
    fn empty_and_malformed_ca_bundles_fail() {
        assert_eq!(
            OutboundPeerVerifier::new(b"", agent_identity()).unwrap_err(),
            VerifierBuildError::InvalidCa
        );
        assert_eq!(
            OutboundPeerVerifier::new(b"secret-canary-invalid-pem", agent_identity()).unwrap_err(),
            VerifierBuildError::InvalidCa
        );
    }

    #[test]
    fn controller_verifier_accepts_any_instance_of_its_cluster_only() {
        let expected = WorkloadIdentity::Controller {
            cluster_id: "cluster-1".to_owned(),
            instance_id: String::new(),
        };
        let controller = |cluster: &str, instance: &str| WorkloadIdentity::Controller {
            cluster_id: cluster.to_owned(),
            instance_id: instance.to_owned(),
        };
        assert!(identity_accepted(
            &expected,
            true,
            &controller("cluster-1", "dev")
        ));
        assert!(identity_accepted(
            &expected,
            true,
            &controller("cluster-1", "replica-2")
        ));
        assert!(!identity_accepted(
            &expected,
            true,
            &controller("cluster-2", "dev")
        ));
        assert!(!identity_accepted(&expected, true, &agent_identity()));
        // Exact matching is unchanged for every other verifier.
        assert!(!identity_accepted(
            &expected,
            false,
            &controller("cluster-1", "dev")
        ));

        assert_eq!(
            OutboundPeerVerifier::for_controller_of(b"not-a-ca", "cluster/other").unwrap_err(),
            VerifierBuildError::InvalidIdentity
        );
        assert_eq!(
            OutboundPeerVerifier::for_controller_of(b"", "cluster-1").unwrap_err(),
            VerifierBuildError::InvalidCa
        );
    }

    #[test]
    fn build_errors_stay_generic() {
        assert_eq!(
            VerifierBuildError::InvalidCa.to_string(),
            "peer CA bundle is invalid"
        );
        assert_eq!(
            VerifierBuildError::InvalidIdentity.to_string(),
            "expected peer identity is invalid"
        );
    }
}
