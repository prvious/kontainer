//! Credential wrappers, redaction, and PKCE derivation.

use crate::credentials::{AccessCredential, CredentialError, LoginVerifier, RefreshCredential};

const ACCESS: &str = "kta_11111111111111111111111111111111.2222222222222222222222222222222222222222222222222222222222222222";
const REFRESH: &str = "ktr_33333333333333333333333333333333.4444444444444444444444444444444444444444444444444444444444444444";
/// RFC 7636 Appendix B verifier.
const RFC_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
/// RFC 7636 Appendix B S256 challenge.
const RFC_CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

#[test]
fn credentials_validate_their_canonical_shape() {
    assert_eq!(AccessCredential::parse(ACCESS).unwrap().expose(), ACCESS);
    assert_eq!(RefreshCredential::parse(REFRESH).unwrap().expose(), REFRESH);

    let uppercase = ACCESS.to_ascii_uppercase();
    let short_id = format!("kta_{}.{}", "1".repeat(31), "2".repeat(64));
    let short_secret = format!("kta_{}.{}", "1".repeat(32), "2".repeat(63));
    for invalid in [
        "",
        "kta_",
        "kta_no-separator",
        "kta_11111111111111111111111111111111",
        short_id.as_str(),
        short_secret.as_str(),
        uppercase.as_str(),
        REFRESH,
    ] {
        assert_eq!(
            AccessCredential::parse(invalid).err(),
            Some(CredentialError),
            "{invalid} must be rejected"
        );
    }
    assert_eq!(
        RefreshCredential::parse(ACCESS).err(),
        Some(CredentialError)
    );
}

#[test]
fn credential_errors_do_not_echo_the_input() {
    let error = AccessCredential::parse("kta_secret-canary").unwrap_err();
    assert_eq!(error.to_string(), "credential is malformed");
    assert!(!error.to_string().contains("canary"));
}

#[test]
fn credentials_are_redacted_in_diagnostics() {
    let access = AccessCredential::parse(ACCESS).unwrap();
    assert_eq!(format!("{access:?}"), "AccessCredential(<redacted>)");
    assert_eq!(format!("{access}"), "AccessCredential(<redacted>)");
    assert!(!format!("{access:?}").contains("2222222222222222"));

    let refresh = RefreshCredential::parse(REFRESH).unwrap();
    assert_eq!(format!("{refresh:?}"), "RefreshCredential(<redacted>)");
    assert_eq!(format!("{refresh}"), "RefreshCredential(<redacted>)");
    assert!(!format!("{refresh:?}").contains("4444444444444444"));

    let verifier = LoginVerifier::parse(RFC_VERIFIER).unwrap();
    assert_eq!(format!("{verifier:?}"), "LoginVerifier(<redacted>)");
    assert_eq!(format!("{verifier}"), "LoginVerifier(<redacted>)");
    assert!(!format!("{verifier:?}").contains(RFC_VERIFIER));
}

#[test]
fn s256_challenge_matches_the_rfc7636_vector() {
    let verifier = LoginVerifier::parse(RFC_VERIFIER).unwrap();
    assert_eq!(verifier.expose(), RFC_VERIFIER);
    assert_eq!(verifier.s256_challenge(), RFC_CHALLENGE);
}

#[test]
fn generated_verifiers_are_rfc7636_shaped_and_distinct() {
    let first = LoginVerifier::generate();
    let second = LoginVerifier::generate();
    let first_value = first.expose().to_owned();
    let second_value = second.expose().to_owned();

    assert_eq!(first_value.len(), 43);
    assert!(
        first_value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')),
        "generated verifiers use only unreserved characters"
    );
    assert_ne!(first_value, second_value);
    // The generated shape passes the same validator applied to parsed input.
    assert!(LoginVerifier::parse(&first_value).is_ok());
    assert_ne!(first.s256_challenge(), second.s256_challenge());
}

#[test]
fn verifier_parsing_enforces_rfc7636() {
    assert!(LoginVerifier::parse(&"a".repeat(43)).is_ok());
    assert!(LoginVerifier::parse(&"a".repeat(128)).is_ok());
    for invalid in [
        String::new(),
        "a".repeat(42),
        "a".repeat(129),
        format!("{}+{}", "a".repeat(21), "a".repeat(21)),
        format!("{}={}", "a".repeat(21), "a".repeat(21)),
        format!("{} {}", "a".repeat(21), "a".repeat(21)),
    ] {
        assert_eq!(
            LoginVerifier::parse(&invalid).err(),
            Some(CredentialError),
            "{} characters must be rejected",
            invalid.len()
        );
    }
}
