use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use rustls::client::danger::ServerCertVerifier;
use rustls::{
    RootCertStore,
    pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
    server::WebPkiClientVerifier,
};
use tonic::transport::{Certificate, ClientTlsConfig, Identity, ServerTlsConfig};

use crate::{OutboundPeerVerifier, WorkloadIdentity};

#[derive(Debug, Clone)]
pub struct MutualTlsIdentityFiles {
    certificate_path: PathBuf,
    private_key_path: PathBuf,
    peer_ca_path: PathBuf,
}

impl MutualTlsIdentityFiles {
    #[must_use]
    pub fn new(
        certificate_path: impl Into<PathBuf>,
        private_key_path: impl Into<PathBuf>,
        peer_ca_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            certificate_path: certificate_path.into(),
            private_key_path: private_key_path.into(),
            peer_ca_path: peer_ca_path.into(),
        }
    }

    pub fn server_config(&self) -> io::Result<ServerTlsConfig> {
        let identity = self.load_identity()?;
        let peer_ca = self.load_peer_ca()?;
        Ok(ServerTlsConfig::new()
            .identity(identity)
            .client_ca_root(peer_ca))
    }

    /// Prepare the mandatory mTLS identity before either listener admits work.
    pub fn server_acceptor(&self) -> io::Result<tokio_rustls::TlsAcceptor> {
        let certificate = read_rpc_tls_file("certificate", &self.certificate_path)?;
        let private_key = read_rpc_tls_file("private key", &self.private_key_path)?;
        let peer_ca = read_rpc_tls_file("client CA", &self.peer_ca_path)?;
        let chain: Vec<_> = CertificateDer::pem_slice_iter(&certificate)
            .collect::<Result<_, _>>()
            .map_err(|_| invalid_rpc_tls_file("certificate", &self.certificate_path))?;
        if chain.is_empty() {
            return Err(invalid_rpc_tls_file("certificate", &self.certificate_path));
        }
        let key = PrivateKeyDer::from_pem_slice(&private_key)
            .map_err(|_| invalid_rpc_tls_file("private key", &self.private_key_path))?;
        let mut roots = RootCertStore::empty();
        for certificate in CertificateDer::pem_slice_iter(&peer_ca) {
            roots
                .add(
                    certificate
                        .map_err(|_| invalid_rpc_tls_file("client CA", &self.peer_ca_path))?,
                )
                .map_err(|_| invalid_rpc_tls_file("client CA", &self.peer_ca_path))?;
        }
        if roots.is_empty() {
            return Err(invalid_rpc_tls_file("client CA", &self.peer_ca_path));
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier =
            WebPkiClientVerifier::builder_with_provider(Arc::new(roots), provider.clone())
                .build()
                .map_err(|_| invalid_rpc_tls_file("client CA", &self.peer_ca_path))?;
        let mut config = rustls::ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(rustls::ALL_VERSIONS)
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid RPC TLS protocol configuration",
                )
            })?
            .with_client_cert_verifier(verifier)
            .with_single_cert(chain, key)
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "invalid RPC certificate/private key pair ({} / {})",
                        self.certificate_path.display(),
                        self.private_key_path.display()
                    ),
                )
            })?;
        config.alpn_protocols.push(b"h2".to_vec());
        Ok(tokio_rustls::TlsAcceptor::from(Arc::new(config)))
    }

    pub fn client_config(
        &self,
        expected_server_name: impl Into<String>,
    ) -> io::Result<ClientTlsConfig> {
        let identity = self.load_identity()?;
        let peer_ca = self.load_peer_ca()?;
        Ok(ClientTlsConfig::new()
            .domain_name(expected_server_name)
            .identity(identity)
            .ca_certificate(peer_ca))
    }

    /// Client TLS config for use with a custom [`OutboundPeerVerifier`].
    ///
    /// Unlike [`Self::client_config`], this sets no CA roots: the custom
    /// verifier owns trust. The caller must combine the returned config with
    /// [`Self::outbound_verifier`] via
    /// `Endpoint::tls_config_with_verifier`, which parses and validates the
    /// certificate material.
    pub fn client_config_for_custom_verifier(
        &self,
        expected_server_name: impl Into<String>,
    ) -> io::Result<ClientTlsConfig> {
        let identity = self.load_identity()?;
        Ok(ClientTlsConfig::new()
            .domain_name(expected_server_name)
            .identity(identity))
    }

    /// Build the identity-bound outbound verifier for `expected`.
    ///
    /// Reads only the peer CA file; certificate and key validation happens
    /// when the Tonic endpoint is built. Errors stay generic and never
    /// include certificate material.
    pub fn outbound_verifier(
        &self,
        expected: WorkloadIdentity,
    ) -> io::Result<Arc<dyn ServerCertVerifier>> {
        let peer_ca_pem = fs::read(&self.peer_ca_path)?;
        let verifier = OutboundPeerVerifier::new(&peer_ca_pem, expected)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        Ok(Arc::new(verifier))
    }

    fn load_identity(&self) -> io::Result<Identity> {
        let certificate = fs::read(&self.certificate_path)?;
        let private_key = fs::read(&self.private_key_path)?;
        Ok(Identity::from_pem(certificate, private_key))
    }

    fn load_peer_ca(&self) -> io::Result<Certificate> {
        fs::read(&self.peer_ca_path).map(Certificate::from_pem)
    }
}

fn invalid_rpc_tls_file(label: &str, path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("invalid RPC {label} ({})", path.display()),
    )
}

fn read_rpc_tls_file(label: &str, path: &Path) -> io::Result<Vec<u8>> {
    fs::read(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("RPC {label} ({}): {error}", path.display()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::MutualTlsIdentityFiles;

    #[test]
    fn debug_output_contains_references_not_certificate_material() {
        let files = MutualTlsIdentityFiles::new(
            "/run/secrets/controller.pem",
            "/run/secrets/controller-key.pem",
            "/run/secrets/agent-ca.pem",
        );
        let debug = format!("{files:?}");

        assert!(debug.contains("/run/secrets/controller.pem"));
        assert!(!debug.contains("BEGIN CERTIFICATE"));
        assert!(!debug.contains("PRIVATE KEY"));
    }
}
