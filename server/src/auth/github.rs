//! GitHub bootstrap identity boundary.
//!
//! The provider seam resolves a GitHub numeric account ID from an
//! authorization code. Upstream access tokens never leave the adapter:
//! they are exchanged, used for one authenticated `/user` lookup, then
//! dropped. Persistence holds only the numeric account ID and mutable
//! display login.

use std::{collections::BTreeSet, path::PathBuf, sync::Arc, time::Duration};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Ten-minute login-attempt lifetime.
pub const LOGIN_ATTEMPT_TTL_MINUTES: i64 = 10;
/// Fixed production GitHub endpoints. Tests target a loopback stub through
/// test-only provider construction, never through settings overrides.
pub const GITHUB_AUTHORIZE_URL: &str = "https://github.com/login/oauth/authorize";
pub const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
pub const GITHUB_USER_URL: &str = "https://api.github.com/user";
/// Response-body ceiling for provider calls.
pub const PROVIDER_BODY_LIMIT: usize = 64 * 1024;
/// Maximum allowlisted identities; bounds configuration parsing.
pub const MAX_ALLOWLISTED_IDS: usize = 100;

const STATE_BYTES: usize = 32;

/// Client secret held as bytes so drop can zeroize. Debug is redacted.
pub struct ClientSecret {
    inner: Vec<u8>,
}

impl ClientSecret {
    #[must_use]
    pub fn new(secret: String) -> Self {
        Self {
            inner: secret.into_bytes(),
        }
    }

    pub fn expose(&self) -> String {
        String::from_utf8_lossy(&self.inner).into_owned()
    }
}

impl std::fmt::Debug for ClientSecret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ClientSecret(<redacted>)")
    }
}

impl Drop for ClientSecret {
    fn drop(&mut self) {
        self.inner.fill(0);
    }
}

/// Identity resolved from GitHub's authenticated user API. The numeric
/// account ID is authoritative; login is mutable display metadata only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubAccount {
    pub account_id: i64,
    pub login: String,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum GitHubProviderError {
    #[error("github authentication failed")]
    Denied,
    #[error("github service unavailable")]
    Unavailable,
}

#[async_trait::async_trait]
pub trait GitHubIdentityProvider: Send + Sync + std::fmt::Debug {
    async fn resolve_identity(
        &self,
        code: &str,
        verifier: &str,
        redirect_uri: &str,
    ) -> Result<GitHubAccount, GitHubProviderError>;
}

/// Production adapter: exact token exchange plus one `/user` lookup.
pub struct ReqwestGitHubProvider {
    client: reqwest::Client,
    client_id: String,
    client_secret: ClientSecret,
    token_url: String,
    user_url: String,
}

impl std::fmt::Debug for ReqwestGitHubProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReqwestGitHubProvider")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .field("token_url", &self.token_url)
            .field("user_url", &self.user_url)
            .finish()
    }
}

impl ReqwestGitHubProvider {
    pub fn new(
        client_id: String,
        client_secret: ClientSecret,
        token_url: String,
        user_url: String,
    ) -> Result<Self, GitHubProviderError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .user_agent("kontainer-controller")
            .build()
            .map_err(|_| GitHubProviderError::Unavailable)?;
        Ok(Self {
            client,
            client_id,
            client_secret,
            token_url,
            user_url,
        })
    }

    #[cfg(test)]
    pub fn endpoint_for_tests(
        client_id: String,
        client_secret: ClientSecret,
        base_url: &str,
    ) -> Result<Self, GitHubProviderError> {
        let base = base_url.trim_end_matches('/');
        Self::new(
            client_id,
            client_secret,
            format!("{base}/login/oauth/access_token"),
            format!("{base}/user"),
        )
    }
}

#[async_trait::async_trait]
impl GitHubIdentityProvider for ReqwestGitHubProvider {
    async fn resolve_identity(
        &self,
        code: &str,
        verifier: &str,
        redirect_uri: &str,
    ) -> Result<GitHubAccount, GitHubProviderError> {
        // Bearer material stays in this function; nothing is logged or
        // persisted. Any failure maps to a stable generic error.
        let secret = self.client_secret.expose();
        let token = bounded_json(
            self.client
                .post(&self.token_url)
                .header("Accept", "application/json")
                .form(&[
                    ("client_id", self.client_id.as_str()),
                    ("client_secret", secret.as_str()),
                    ("code", code),
                    ("redirect_uri", redirect_uri),
                    ("code_verifier", verifier),
                ])
                .send()
                .await
                .map_err(|_| GitHubProviderError::Unavailable)?,
        )
        .await?;
        let access_token = token
            .get("access_token")
            .and_then(serde_json::Value::as_str)
            .ok_or(GitHubProviderError::Denied)?;
        if access_token.is_empty() || access_token.len() > 512 {
            return Err(GitHubProviderError::Denied);
        }

        let user = bounded_json(
            self.client
                .get(&self.user_url)
                .header("Accept", "application/vnd.github+json")
                .header("Authorization", format!("Bearer {access_token}"))
                .send()
                .await
                .map_err(|_| GitHubProviderError::Unavailable)?,
        )
        .await?;
        let account_id = user
            .get("id")
            .and_then(serde_json::Value::as_i64)
            .ok_or(GitHubProviderError::Denied)?;
        let login = user
            .get("login")
            .and_then(serde_json::Value::as_str)
            .ok_or(GitHubProviderError::Denied)?;
        validate_account(account_id, login).ok_or(GitHubProviderError::Denied)?;
        Ok(GitHubAccount {
            account_id,
            login: login.to_owned(),
        })
    }
}

/// Read a provider response into bounded JSON. Transport failures and
/// upstream 5xx are outages; other error statuses and malformed bodies are
/// authentication denials. The body streams in chunks so a hostile response
/// cannot allocate past the limit before it is rejected.
async fn bounded_json(
    response: reqwest::Response,
) -> Result<serde_json::Value, GitHubProviderError> {
    let status = response.status();
    if status.is_server_error() {
        return Err(GitHubProviderError::Unavailable);
    }
    if !status.is_success() {
        return Err(GitHubProviderError::Denied);
    }
    let mut buffered = Vec::new();
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| GitHubProviderError::Unavailable)?
    {
        if buffered.len() + chunk.len() > PROVIDER_BODY_LIMIT {
            return Err(GitHubProviderError::Unavailable);
        }
        buffered.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&buffered).map_err(|_| GitHubProviderError::Denied)
}

fn validate_account(account_id: i64, login: &str) -> Option<()> {
    if account_id <= 0 {
        return None;
    }
    let login = login.trim();
    if login.is_empty() || login.len() > 255 {
        return None;
    }
    Some(())
}

/// Typed `github_auth` settings. Unknown fields are rejected; secrets travel
/// by file path only and never appear in error text.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct GitHubAuthSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret_path: Option<PathBuf>,
    #[serde(default)]
    pub allowlisted_account_ids: String,
}

impl GitHubAuthSettings {
    /// Secret-file reference, if one is configured. An empty path from an
    /// unset environment default counts as missing.
    pub fn secret_path(&self) -> Option<&std::path::Path> {
        self.client_secret_path
            .as_ref()
            .filter(|path| !path.as_os_str().is_empty())
            .map(std::path::PathBuf::as_path)
    }

    /// Validate shape without echoing secret material.
    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        if self.client_id.is_empty()
            || self.client_id.len() > 255
            || !self
                .client_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err("github_auth client_id is invalid".to_owned());
        }
        if self.secret_path().is_none() {
            return Err("github_auth client_secret_path is required".to_owned());
        }
        self.allowlist()?;
        Ok(())
    }

    pub fn allowlist(&self) -> Result<BTreeSet<i64>, String> {
        parse_allowlisted_ids(&self.allowlisted_account_ids)
    }
}

pub fn parse_allowlisted_ids(raw: &str) -> Result<BTreeSet<i64>, String> {
    let mut ids = BTreeSet::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let id: i64 =
            part.parse().ok().filter(|id| *id > 0).ok_or_else(|| {
                "github_auth allowlist holds positive numeric account IDs".to_owned()
            })?;
        ids.insert(id);
    }
    if ids.is_empty() || ids.len() > MAX_ALLOWLISTED_IDS {
        return Err("github_auth allowlist holds positive numeric account IDs".to_owned());
    }
    Ok(ids)
}

/// Runtime login configuration: allowlist plus fixed endpoints.
#[derive(Debug, Clone)]
pub struct GitHubLoginConfig {
    pub enabled: bool,
    pub client_id: String,
    pub allowlisted: BTreeSet<i64>,
    pub authorize_url: String,
    pub token_url: String,
    pub user_url: String,
}

impl GitHubLoginConfig {
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            client_id: String::new(),
            allowlisted: BTreeSet::new(),
            authorize_url: GITHUB_AUTHORIZE_URL.to_owned(),
            token_url: GITHUB_TOKEN_URL.to_owned(),
            user_url: GITHUB_USER_URL.to_owned(),
        }
    }
}

/// Composition root for login handlers: config plus provider client.
#[derive(Debug, Clone)]
pub struct GitHubAuthService {
    pub config: GitHubLoginConfig,
    pub provider: Arc<dyn GitHubIdentityProvider>,
}

impl GitHubAuthService {
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            config: GitHubLoginConfig::disabled(),
            provider: Arc::new(UnavailableProvider),
        }
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

#[derive(Debug)]
struct UnavailableProvider;

#[async_trait::async_trait]
impl GitHubIdentityProvider for UnavailableProvider {
    async fn resolve_identity(
        &self,
        _code: &str,
        _verifier: &str,
        _redirect_uri: &str,
    ) -> Result<GitHubAccount, GitHubProviderError> {
        Err(GitHubProviderError::Unavailable)
    }
}

/// Exact callback URL from the canonical controller origin. Never derived
/// from request Host or forwarding headers.
pub fn callback_url(controller_origin: &str) -> Result<String, String> {
    let origin = controller_origin.trim_end_matches('/');
    let joined = format!("{origin}/api/v1/auth/github/callback");
    let parsed = url::Url::parse(&joined).map_err(|_| "controller origin is invalid".to_owned())?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err("controller origin is invalid".to_owned()),
    }
    if parsed.username() != "" || parsed.password().is_some() {
        return Err("controller origin is invalid".to_owned());
    }
    Ok(joined)
}

/// Authorization URL sent to the desktop. Carries only GitHub OAuth
/// parameters, state, and the S256 challenge: never the verifier or a
/// Kontainer credential.
pub fn authorize_url(
    config: &GitHubLoginConfig,
    redirect_uri: &str,
    state: &str,
    challenge: &str,
) -> String {
    let mut url = url::Url::parse(&config.authorize_url).expect("provider endpoint is valid");
    url.query_pairs_mut()
        .append_pair("client_id", &config.client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("state", state)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("allow_signup", "false");
    url.into()
}

/// 256-bit random OAuth state plus its SHA-256 persistence hash.
pub fn generate_oauth_state() -> (String, String) {
    let mut bytes = [0u8; STATE_BYTES];
    rustls::crypto::ring::default_provider()
        .secure_random
        .fill(&mut bytes)
        .expect("ring secure random must succeed");
    let plaintext = URL_SAFE_NO_PAD.encode(bytes);
    let hash = format!("{:x}", Sha256::digest(plaintext.as_bytes()));
    (plaintext, hash)
}

pub fn state_hash(plaintext: &str) -> String {
    format!("{:x}", Sha256::digest(plaintext.as_bytes()))
}

fn is_base64url_nopad(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// Desktop S256 challenges are 43-character base64url digests.
#[must_use]
pub fn valid_challenge(challenge: &str) -> bool {
    challenge.len() == 43 && is_base64url_nopad(challenge)
}

/// RFC 7636 verifiers: 43-128 characters from the unreserved set.
#[must_use]
pub fn valid_verifier(verifier: &str) -> bool {
    (43..=128).contains(&verifier.len())
        && verifier.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || byte == b'-'
                || byte == b'_'
                || byte == b'.'
                || byte == b'~'
        })
}

/// Recompute the S256 challenge and compare in constant time.
#[must_use]
pub fn challenge_matches_verifier(challenge: &str, verifier: &str) -> bool {
    if !valid_challenge(challenge) || !valid_verifier(verifier) {
        return false;
    }
    s256_challenge(verifier)
        .as_bytes()
        .ct_eq(challenge.as_bytes())
        .into()
}

/// RFC 7636 S256 challenge for a verifier. Test fixtures use this to mint
/// matching pairs without duplicating the transform.
#[must_use]
pub fn s256_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[cfg(test)]
pub mod fake {
    use super::{GitHubAccount, GitHubIdentityProvider, GitHubProviderError};
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    /// Deterministic provider stub. Tests assert the adapter contract
    /// (exchange-before-user, verifier forwarding) through a loopback HTTP
    /// stub instead; this fake covers action-level races cheaply.
    #[derive(Debug, Clone)]
    pub struct FakeGitHubProvider {
        pub account: GitHubAccount,
        pub error: Option<GitHubProviderError>,
        pub routes: Arc<Mutex<HashMap<String, GitHubAccount>>>,
        pub calls: Arc<Mutex<Vec<(String, String, String)>>>,
    }

    impl FakeGitHubProvider {
        #[must_use]
        pub fn succeed(account_id: i64, login: &str) -> Self {
            Self {
                account: GitHubAccount {
                    account_id,
                    login: login.to_owned(),
                },
                error: None,
                routes: Arc::new(Mutex::new(HashMap::new())),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        #[must_use]
        pub fn fail(error: GitHubProviderError) -> Self {
            Self {
                account: GitHubAccount {
                    account_id: 1,
                    login: "nobody".to_owned(),
                },
                error: Some(error),
                routes: Arc::new(Mutex::new(HashMap::new())),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        /// Resolve `code` to a distinct account, so race tests can redeem
        /// two attempts as two identities against one service.
        #[must_use]
        pub fn route(self, code: &str, account_id: i64, login: &str) -> Self {
            self.routes.lock().unwrap().insert(
                code.to_owned(),
                GitHubAccount {
                    account_id,
                    login: login.to_owned(),
                },
            );
            self
        }
    }

    #[async_trait::async_trait]
    impl GitHubIdentityProvider for FakeGitHubProvider {
        async fn resolve_identity(
            &self,
            code: &str,
            verifier: &str,
            redirect_uri: &str,
        ) -> Result<GitHubAccount, GitHubProviderError> {
            self.calls.lock().unwrap().push((
                code.to_owned(),
                verifier.to_owned(),
                redirect_uri.to_owned(),
            ));
            if let Some(error) = &self.error {
                return Err(error.clone());
            }
            if let Some(account) = self.routes.lock().unwrap().get(code) {
                return Ok(account.clone());
            }
            Ok(self.account.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        authorize_url, callback_url, challenge_matches_verifier, generate_oauth_state,
        parse_allowlisted_ids, valid_challenge, valid_verifier,
    };
    use crate::auth::github::GitHubLoginConfig;

    #[test]
    fn rfc7636_vector_matches() {
        // RFC 7636 appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(valid_verifier(verifier));
        assert!(valid_challenge(challenge));
        assert!(challenge_matches_verifier(challenge, verifier));
        assert!(!challenge_matches_verifier(
            challenge,
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXj"
        ));
    }

    #[test]
    fn generated_state_is_url_safe_and_hashed() {
        let (plaintext, hash) = generate_oauth_state();
        assert_eq!(plaintext.len(), 43);
        assert!(valid_challenge(&plaintext));
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn allowlist_requires_positive_numeric_ids() {
        let ids = parse_allowlisted_ids("42, 7,42").unwrap();
        assert_eq!(ids.into_iter().collect::<Vec<_>>(), vec![7, 42]);
        for invalid in ["", "0", "-3", "abc", "1,1,not-a-number"] {
            assert!(parse_allowlisted_ids(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn callback_url_rejects_non_http_origins() {
        assert_eq!(
            callback_url("https://controller.example").unwrap(),
            "https://controller.example/api/v1/auth/github/callback"
        );
        for invalid in [
            "not-a-url",
            "ftp://controller.example",
            "https://user@example.com",
        ] {
            assert!(callback_url(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn authorize_url_carries_no_verifier_or_kontainer_credential() {
        let config = GitHubLoginConfig {
            enabled: true,
            client_id: "client-id".to_owned(),
            allowlisted: [1].into_iter().collect(),
            authorize_url: super::GITHUB_AUTHORIZE_URL.to_owned(),
            token_url: super::GITHUB_TOKEN_URL.to_owned(),
            user_url: super::GITHUB_USER_URL.to_owned(),
        };
        let url = authorize_url(
            &config,
            "https://ctl/callback",
            "opaque-state",
            "challenge-value",
        );
        assert!(url.contains("code_challenge_method=S256"));
        assert!(!url.contains("code_verifier"));
        assert!(!url.contains("kta_") && !url.contains("ktr_"));
    }

    #[test]
    fn client_secret_debug_is_redacted() {
        let secret = super::ClientSecret::new("secret-canary".to_owned());
        assert!(!format!("{secret:?}").contains("secret-canary"));
        assert!(!secret.expose().is_empty());
    }
}
