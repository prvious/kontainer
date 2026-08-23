use std::{fs, path::PathBuf};

use kontainer_rpc_auth::MutualTlsIdentityFiles;
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa,
    Issuer, KeyPair, KeyUsagePurpose, SanType,
};
use tempfile::TempDir;
use tonic::transport::{Certificate as TonicCertificate, ClientTlsConfig};

pub const CLUSTER_ID: &str = "cluster-test";
pub const OTHER_CLUSTER_ID: &str = "cluster-other";
pub const NODE_ID: &str = "node-1";
pub const CONTROLLER_NAME: &str = "controller.kontainer.test";
pub const NODE_AGENT_NAME: &str = "node-1.agent.kontainer.test";
pub const CONTROLLER_URI: &str =
    "spiffe://kontainer.internal/swarm/cluster-test/controller/controller-a";
pub const AGENT_URI: &str = "spiffe://kontainer.internal/swarm/cluster-test/node/node-1/agent";

#[derive(Debug, Clone)]
pub struct IdentityFixture {
    certificate_path: PathBuf,
    private_key_path: PathBuf,
}

pub struct TestPki {
    _directory: TempDir,
    trusted_ca_path: PathBuf,
    pub controller: IdentityFixture,
    pub agent: IdentityFixture,
    pub wrong_workload: IdentityFixture,
    pub wrong_cluster: IdentityFixture,
    pub untrusted_agent: IdentityFixture,
}

impl TestPki {
    pub fn generate() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let (trusted_ca, trusted_issuer) = certificate_authority("Kontainer test CA");
        let (untrusted_ca, untrusted_issuer) = certificate_authority("Unknown test CA");
        let trusted_ca_path = directory.path().join("trusted-ca.pem");
        let untrusted_ca_path = directory.path().join("untrusted-ca.pem");
        fs::write(&trusted_ca_path, trusted_ca.pem()).unwrap();
        fs::write(&untrusted_ca_path, untrusted_ca.pem()).unwrap();

        let controller = issue_identity(
            directory.path(),
            "controller",
            CONTROLLER_NAME,
            CONTROLLER_URI,
            &trusted_issuer,
        );
        let agent = issue_identity(
            directory.path(),
            "agent",
            NODE_AGENT_NAME,
            AGENT_URI,
            &trusted_issuer,
        );
        let wrong_workload = issue_identity(
            directory.path(),
            "wrong-workload",
            "wrong-workload.kontainer.test",
            CONTROLLER_URI,
            &trusted_issuer,
        );
        let wrong_cluster = issue_identity(
            directory.path(),
            "wrong-cluster",
            "wrong-cluster.kontainer.test",
            "spiffe://kontainer.internal/swarm/cluster-other/node/node-1/agent",
            &trusted_issuer,
        );
        let untrusted_agent = issue_identity(
            directory.path(),
            "untrusted-agent",
            "untrusted-agent.kontainer.test",
            AGENT_URI,
            &untrusted_issuer,
        );

        Self {
            _directory: directory,
            trusted_ca_path,
            controller,
            agent,
            wrong_workload,
            wrong_cluster,
            untrusted_agent,
        }
    }

    pub fn trusted_files(&self, identity: &IdentityFixture) -> MutualTlsIdentityFiles {
        MutualTlsIdentityFiles::new(
            &identity.certificate_path,
            &identity.private_key_path,
            &self.trusted_ca_path,
        )
    }

    pub fn untrusted_agent_files(&self) -> MutualTlsIdentityFiles {
        MutualTlsIdentityFiles::new(
            &self.untrusted_agent.certificate_path,
            &self.untrusted_agent.private_key_path,
            &self.trusted_ca_path,
        )
    }

    pub fn anonymous_client_config(&self, server_name: &str) -> ClientTlsConfig {
        let trusted_ca = fs::read(&self.trusted_ca_path).unwrap();
        ClientTlsConfig::new()
            .domain_name(server_name)
            .ca_certificate(TonicCertificate::from_pem(trusted_ca))
    }
}

fn certificate_authority(common_name: &str) -> (Certificate, Issuer<'static, KeyPair>) {
    let mut parameters = CertificateParams::new(Vec::new()).unwrap();
    parameters
        .distinguished_name
        .push(DnType::CommonName, common_name);
    parameters.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    parameters.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
    ];
    let key_pair = KeyPair::generate().unwrap();
    let certificate = parameters.self_signed(&key_pair).unwrap();
    let issuer = Issuer::new(parameters, key_pair);
    (certificate, issuer)
}

fn issue_identity(
    directory: &std::path::Path,
    fixture_name: &str,
    dns_name: &str,
    workload_uri: &str,
    issuer: &Issuer<'_, KeyPair>,
) -> IdentityFixture {
    let mut parameters = CertificateParams::new(vec![dns_name.to_owned()]).unwrap();
    parameters
        .distinguished_name
        .push(DnType::CommonName, fixture_name);
    parameters
        .subject_alt_names
        .push(SanType::URI(workload_uri.try_into().unwrap()));
    parameters.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    parameters.extended_key_usages = vec![
        ExtendedKeyUsagePurpose::ClientAuth,
        ExtendedKeyUsagePurpose::ServerAuth,
    ];
    parameters.use_authority_key_identifier_extension = true;

    let key_pair = KeyPair::generate().unwrap();
    let certificate = parameters.signed_by(&key_pair, issuer).unwrap();
    let certificate_path = directory.join(format!("{fixture_name}.pem"));
    let private_key_path = directory.join(format!("{fixture_name}-key.pem"));
    fs::write(&certificate_path, certificate.pem()).unwrap();
    fs::write(&private_key_path, key_pair.serialize_pem()).unwrap();

    IdentityFixture {
        certificate_path,
        private_key_path,
    }
}
