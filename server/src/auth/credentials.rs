//! Opaque session credentials.
//!
//! Both credential kinds carry 256 bits of CSPRNG entropy from the explicit
//! ring provider and are returned to the caller exactly once. Persistence
//! holds only SHA-256 hashes of the complete canonical credential string.
//! `Debug` is redacted by construction so credentials never reach logs.

use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const ACCESS_PREFIX: &str = "kta_";
pub const REFRESH_PREFIX: &str = "ktr_";
const SECRET_BYTES: usize = 32;
const SECRET_HEX_LEN: usize = 64;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("credential is malformed")]
pub struct CredentialParseError;

pub struct AccessCredential {
    public_id: Uuid,
    secret: [u8; SECRET_BYTES],
}

pub struct RefreshCredential {
    public_id: Uuid,
    secret: [u8; SECRET_BYTES],
}

impl std::fmt::Debug for AccessCredential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AccessCredential(<redacted>)")
    }
}

impl std::fmt::Debug for RefreshCredential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RefreshCredential(<redacted>)")
    }
}

impl Drop for AccessCredential {
    fn drop(&mut self) {
        self.secret.fill(0);
    }
}

impl Drop for RefreshCredential {
    fn drop(&mut self) {
        self.secret.fill(0);
    }
}

fn random_secret() -> [u8; SECRET_BYTES] {
    let mut secret = [0u8; SECRET_BYTES];
    rustls::crypto::ring::default_provider()
        .secure_random
        .fill(&mut secret)
        .expect("ring secure random must succeed");
    secret
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_secret(hex: &str) -> Option<[u8; SECRET_BYTES]> {
    if hex.len() != SECRET_HEX_LEN || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    // Reject uppercase: canonical credentials are lowercase hex only.
    if hex.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return None;
    }
    let mut secret = [0u8; SECRET_BYTES];
    for (index, chunk) in hex.as_bytes().chunks(2).enumerate() {
        secret[index] = u8::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
    }
    Some(secret)
}

fn split_credential(value: &str, prefix: &str) -> Option<(Uuid, [u8; SECRET_BYTES])> {
    let rest = value.strip_prefix(prefix)?;
    let (id_part, secret_part) = rest.split_once('.')?;
    if secret_part.len() != SECRET_HEX_LEN {
        return None;
    }
    let public_id = Uuid::parse_str(id_part).ok()?;
    let secret = decode_secret(secret_part)?;
    Some((public_id, secret))
}

pub fn credential_hash(canonical: &str) -> String {
    format!("{:x}", Sha256::digest(canonical.as_bytes()))
}

impl AccessCredential {
    #[must_use]
    pub fn generate() -> Self {
        Self {
            public_id: Uuid::now_v7(),
            secret: random_secret(),
        }
    }

    pub fn parse(value: &str) -> Result<Self, CredentialParseError> {
        split_credential(value, ACCESS_PREFIX)
            .map(|(public_id, secret)| Self { public_id, secret })
            .ok_or(CredentialParseError)
    }

    #[must_use]
    pub fn public_id(&self) -> Uuid {
        self.public_id
    }

    #[must_use]
    pub fn plaintext(&self) -> String {
        format!(
            "{ACCESS_PREFIX}{}.{secret}",
            self.public_id.simple(),
            secret = encode_hex(&self.secret)
        )
    }

    #[must_use]
    pub fn credential_hash(&self) -> String {
        credential_hash(&self.plaintext())
    }
}

impl RefreshCredential {
    #[must_use]
    pub fn generate() -> Self {
        Self {
            public_id: Uuid::now_v7(),
            secret: random_secret(),
        }
    }

    pub fn parse(value: &str) -> Result<Self, CredentialParseError> {
        split_credential(value, REFRESH_PREFIX)
            .map(|(public_id, secret)| Self { public_id, secret })
            .ok_or(CredentialParseError)
    }

    #[must_use]
    pub fn public_id(&self) -> Uuid {
        self.public_id
    }

    #[must_use]
    pub fn plaintext(&self) -> String {
        format!(
            "{REFRESH_PREFIX}{}.{secret}",
            self.public_id.simple(),
            secret = encode_hex(&self.secret)
        )
    }

    #[must_use]
    pub fn credential_hash(&self) -> String {
        credential_hash(&self.plaintext())
    }
}

/// Hash a presented credential string without parsing it into a secret type.
/// Used by persistence lookups so malformed input fails before any query.
#[must_use]
pub fn hash_presented(value: &str) -> String {
    credential_hash(value)
}

#[cfg(test)]
mod tests {
    use super::{AccessCredential, RefreshCredential, credential_hash};

    #[test]
    fn issued_credentials_are_distinct_and_well_formed() {
        let first = AccessCredential::generate();
        let second = AccessCredential::generate();
        assert_ne!(first.plaintext(), second.plaintext());

        let parsed = AccessCredential::parse(&first.plaintext()).unwrap();
        assert_eq!(parsed.public_id(), first.public_id());
        assert_eq!(parsed.credential_hash(), first.credential_hash());

        let refresh = RefreshCredential::generate();
        assert!(refresh.plaintext().starts_with("ktr_"));
        assert!(AccessCredential::parse(&refresh.plaintext()).is_err());
        assert!(RefreshCredential::parse(&first.plaintext()).is_err());
    }

    #[test]
    fn credential_debug_is_redacted() {
        let access = AccessCredential::generate();
        let plaintext = access.plaintext();
        assert!(!format!("{access:?}").contains(&plaintext));
        assert!(!format!("{access:?}").contains("kta_"));

        let refresh = RefreshCredential::generate();
        let plaintext = refresh.plaintext();
        assert!(!format!("{refresh:?}").contains(&plaintext));
    }

    #[test]
    fn malformed_credentials_are_rejected_without_detail() {
        let access = AccessCredential::generate();
        let plaintext = access.plaintext();
        let mut uppercase = plaintext.clone();
        uppercase.make_ascii_uppercase();
        for invalid in [
            String::new(),
            "kta_".to_owned(),
            "kta_no-separator".to_owned(),
            "kta_not-a-uuid.00".to_owned(),
            uppercase,
            plaintext.replace("kta_", "ktr_"),
        ] {
            let error = AccessCredential::parse(&invalid).unwrap_err();
            assert_eq!(error.to_string(), "credential is malformed");
            if !invalid.is_empty() {
                assert!(!error.to_string().contains(&invalid as &str));
            }
        }
    }

    #[test]
    fn hashes_are_stable_hex_without_plaintext() {
        let access = AccessCredential::generate();
        let hash = access.credential_hash();
        assert_eq!(hash.len(), 64);
        assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(hash, credential_hash(&access.plaintext()));
        assert!(!hash.contains(&access.plaintext()));
    }
}
