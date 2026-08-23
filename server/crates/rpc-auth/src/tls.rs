use std::{fs, io, path::PathBuf};

use tonic::transport::{Certificate, ClientTlsConfig, Identity, ServerTlsConfig};

#[derive(Clone)]
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

    fn load_identity(&self) -> io::Result<Identity> {
        let certificate = fs::read(&self.certificate_path)?;
        let private_key = fs::read(&self.private_key_path)?;
        Ok(Identity::from_pem(certificate, private_key))
    }

    fn load_peer_ca(&self) -> io::Result<Certificate> {
        fs::read(&self.peer_ca_path).map(Certificate::from_pem)
    }
}

impl std::fmt::Debug for MutualTlsIdentityFiles {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MutualTlsIdentityFiles")
            .field("certificate_path", &self.certificate_path)
            .field("private_key_path", &self.private_key_path)
            .field("peer_ca_path", &self.peer_ca_path)
            .finish()
    }
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
