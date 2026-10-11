//! Cluster trust from one shared secret (ADR 0045).
//!
//! Every controller and agent given the same `KONTAINER_SECRET` derives the same cluster CA, then
//! signs a fresh certificate for its own workload identity. The files it writes are the ones
//! [`MutualTlsIdentityFiles`] already reads, so every mTLS check stays as it was; only who issues
//! the certificates changes.

use std::{
    fmt, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, DnType, ExtendedKeyUsagePurpose, IsCa,
    KeyPair, KeyUsagePurpose, PKCS_ED25519, SanType, string::Ia5String,
};
use rustls::pki_types::PrivatePkcs8KeyDer;
use sha2::{Digest, Sha256};

use crate::{MutualTlsIdentityFiles, WorkloadIdentity};

/// Cluster id both sides use unless configured otherwise.
pub const DEFAULT_CLUSTER_ID: &str = "default";
/// DNS name a secret-issued controller certificate carries; agents expect it by default.
pub const CONTROLLER_DNS_NAME: &str = "controller.kontainer.internal";
/// Agent certificates carry `{node_id}.{AGENT_DNS_SUFFIX}`.
pub const AGENT_DNS_SUFFIX: &str = "agent.kontainer.internal";
/// Shortest secret accepted: the CA key is derived from it, so it must resist guessing.
/// `openssl rand -hex 32` prints 64 characters.
pub const MINIMUM_SECRET_LENGTH: usize = 32;

/// PKCS#8 v1 header for a raw Ed25519 seed (RFC 8410 section 7).
const ED25519_PKCS8_PREFIX: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

/// The shared secret. Its value never appears in `Debug` output or errors.
#[derive(Clone)]
pub struct ClusterSecret(String);

impl fmt::Debug for ClusterSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ClusterSecret(redacted)")
    }
}

impl ClusterSecret {
    /// Trailing newlines are dropped: secret files usually end with one.
    pub fn new(value: &str) -> Result<Self, String> {
        let value = value.trim_end_matches(['\r', '\n']);
        if value.chars().count() < MINIMUM_SECRET_LENGTH {
            return Err(format!(
                "KONTAINER_SECRET must be at least {MINIMUM_SECRET_LENGTH} characters; generate one with `openssl rand -hex 32`"
            ));
        }
        Ok(Self(value.to_owned()))
    }

    /// `KONTAINER_SECRET`, or the file `KONTAINER_SECRET_FILE` names (a Swarm secret). `None`
    /// when neither is set.
    pub fn from_environment() -> Result<Option<Self>, String> {
        Self::from_sources(
            std::env::var("KONTAINER_SECRET").ok(),
            std::env::var_os("KONTAINER_SECRET_FILE").map(PathBuf::from),
        )
    }

    fn from_sources(value: Option<String>, file: Option<PathBuf>) -> Result<Option<Self>, String> {
        let value = match (value, file) {
            (Some(_), Some(_)) => {
                return Err("set KONTAINER_SECRET or KONTAINER_SECRET_FILE, not both".to_owned());
            }
            (Some(value), None) => value,
            (None, Some(path)) => fs::read_to_string(&path).map_err(|error| {
                format!(
                    "KONTAINER_SECRET_FILE ({}) is unreadable: {error}",
                    path.display()
                )
            })?,
            (None, None) => return Ok(None),
        };
        Self::new(&value).map(Some)
    }

    /// Write a certificate for `identity`, its fresh private key, and the cluster CA into
    /// `directory`, and return them as the files the mTLS code reads. `directory` must not exist
    /// yet: it is created owner-only and its files exclusively, so nothing planted in a shared
    /// temporary directory is followed or reused. [`runtime_directory`] picks a fresh one.
    pub fn issue_identity_files(
        &self,
        identity: &WorkloadIdentity,
        dns_name: &str,
        directory: &Path,
    ) -> io::Result<MutualTlsIdentityFiles> {
        let authority = self.certificate_authority().map_err(invalid)?;
        let key = KeyPair::generate().map_err(invalid)?;
        let mut params = CertificateParams::new(vec![dns_name.to_owned()]).map_err(invalid)?;
        params.distinguished_name.push(DnType::CommonName, dns_name);
        params.subject_alt_names.push(SanType::URI(
            Ia5String::try_from(identity.canonical_uri()).map_err(invalid)?,
        ));
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![
            ExtendedKeyUsagePurpose::ServerAuth,
            ExtendedKeyUsagePurpose::ClientAuth,
        ];
        params.use_authority_key_identifier_extension = true;
        // ponytail: rcgen's default validity runs to 4096 and nothing renews. Trust is the secret
        // itself: anyone holding it can mint any identity, so rotate the secret to revoke.
        let certificate = params.signed_by(&key, &authority).map_err(invalid)?;

        create_private_directory(directory)?;
        let certificate_path = directory.join("certificate.pem");
        let private_key_path = directory.join("private-key.pem");
        let cluster_ca_path = directory.join("cluster-ca.pem");
        write_private_file(&certificate_path, certificate.pem().as_bytes())?;
        write_private_file(&private_key_path, key.serialize_pem().as_bytes())?;
        write_private_file(&cluster_ca_path, authority.pem().as_bytes())?;
        Ok(MutualTlsIdentityFiles::new(
            certificate_path,
            private_key_path,
            cluster_ca_path,
        ))
    }

    /// The same CA on every node: an Ed25519 key seeded from the secret signs a certificate with
    /// fixed fields, and Ed25519 signatures are deterministic.
    fn certificate_authority(&self) -> Result<CertifiedIssuer<'static, KeyPair>, rcgen::Error> {
        let seed = Sha256::new()
            .chain_update(b"kontainer cluster CA v1\0")
            .chain_update(self.0.as_bytes())
            .finalize();
        let mut pkcs8 = ED25519_PKCS8_PREFIX.to_vec();
        pkcs8.extend_from_slice(&seed);
        let key =
            KeyPair::from_pkcs8_der_and_sign_algo(&PrivatePkcs8KeyDer::from(pkcs8), &PKCS_ED25519)?;
        let mut params = CertificateParams::default();
        params
            .distinguished_name
            .push(DnType::CommonName, "Kontainer cluster CA");
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];
        params.serial_number = Some(1_u64.into());
        CertifiedIssuer::self_signed(params, key)
    }
}

fn invalid(error: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

/// A fresh, unused path under the system temporary directory for one process's identity files.
/// The time covers a restarted container reusing its pid; the counter covers calls in one process.
#[must_use]
pub fn runtime_directory(role: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "kontainer-{role}-{}-{nanos}-{sequence}",
        std::process::id()
    ))
}

/// Not recursive: an existing directory, whoever made it, is an error.
fn create_private_directory(directory: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(directory)
}

fn write_private_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(contents)
}

#[cfg(test)]
mod tests {
    use super::{ClusterSecret, MINIMUM_SECRET_LENGTH, runtime_directory};
    use crate::WorkloadIdentity;

    const SECRET: &str = "0123456789abcdef0123456789abcdef";

    fn agent(node_id: &str) -> WorkloadIdentity {
        WorkloadIdentity::NodeAgent {
            cluster_id: "default".to_owned(),
            node_id: node_id.to_owned(),
        }
    }

    #[test]
    fn short_secrets_are_refused_without_echoing_them() {
        let short = "s".repeat(MINIMUM_SECRET_LENGTH - 1);
        let error = ClusterSecret::new(&short).unwrap_err();
        assert!(error.contains("at least 32 characters"));
        assert!(!error.contains(&short));
        // A newline from a secret file does not count toward the length.
        assert!(ClusterSecret::new(&format!("{short}\n")).is_err());
        assert!(ClusterSecret::new(&format!("{SECRET}\n")).is_ok());
        assert_eq!(
            format!("{:?}", ClusterSecret::new(SECRET).unwrap()),
            "ClusterSecret(redacted)"
        );
    }

    #[test]
    fn the_secret_comes_from_the_variable_or_a_file_but_not_both() {
        assert!(ClusterSecret::from_sources(None, None).unwrap().is_none());
        assert!(
            ClusterSecret::from_sources(Some(SECRET.to_owned()), None)
                .unwrap()
                .is_some()
        );

        let directory = runtime_directory("secret-source-test");
        std::fs::create_dir(&directory).unwrap();
        let file = directory.join("secret");
        std::fs::write(&file, format!("{SECRET}\n")).unwrap();
        assert!(
            ClusterSecret::from_sources(None, Some(file.clone()))
                .unwrap()
                .is_some()
        );
        assert!(ClusterSecret::from_sources(Some(SECRET.to_owned()), Some(file)).is_err());
        let missing = ClusterSecret::from_sources(None, Some(directory.join("missing")));
        assert!(missing.unwrap_err().contains("is unreadable"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn every_holder_of_the_secret_derives_the_same_ca_and_others_do_not() {
        let first = ClusterSecret::new(SECRET).unwrap();
        let second = ClusterSecret::new(SECRET).unwrap();
        let other = ClusterSecret::new(&SECRET.replace('0', "9")).unwrap();

        let ca = |secret: &ClusterSecret| secret.certificate_authority().unwrap().pem();
        assert_eq!(ca(&first), ca(&second));
        assert_ne!(ca(&first), ca(&other));
    }

    #[test]
    fn a_planted_directory_is_never_reused() {
        let planted = runtime_directory("planted-test");
        std::fs::create_dir(&planted).unwrap();
        let error = ClusterSecret::new(SECRET)
            .unwrap()
            .issue_identity_files(
                &agent("node-1"),
                "node-1.agent.kontainer.internal",
                &planted,
            )
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(std::fs::read_dir(&planted).unwrap().next().is_none());
        std::fs::remove_dir(&planted).unwrap();
        assert_ne!(runtime_directory("x"), runtime_directory("x"));
    }

    #[test]
    fn issued_files_carry_the_workload_identity_and_stay_private() {
        let directory = runtime_directory("issued-files-test");
        let files = ClusterSecret::new(SECRET)
            .unwrap()
            .issue_identity_files(
                &agent("node-1"),
                "node-1.agent.kontainer.internal",
                &directory,
            )
            .unwrap();

        let certificate = std::fs::read(directory.join("certificate.pem")).unwrap();
        let der = rustls::pki_types::pem::PemObject::from_pem_slice(&certificate)
            .map(|der: rustls::pki_types::CertificateDer<'_>| der.to_vec())
            .unwrap();
        assert_eq!(
            WorkloadIdentity::from_certificate_der(&der).unwrap(),
            agent("node-1")
        );
        assert!(files.server_acceptor().is_ok(), "key matches certificate");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |name: &str| {
                std::fs::metadata(directory.join(name))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777
            };
            assert_eq!(mode(""), 0o700);
            assert_eq!(mode("private-key.pem"), 0o600);
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}
