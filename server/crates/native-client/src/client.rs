//! Controller origin policy, transport construction, and bounded exchange.
//!
//! [`NativeClient`] stores exactly three things: the validated controller
//! origin, the HTTP transport, and the bounded limits. It has no credential
//! field, no default authorization header, and no background tasks.
//!
//! Every call runs inside one absolute deadline and one explicit cancellation
//! token. The single retry for classified pre-response failures happens
//! inside that same deadline, so a retried call can never extend the
//! caller's budget.

use std::time::Duration;

use reqwest::{Client, Method, Request, Response, header, redirect, retry};
use serde::de::DeserializeOwned;
use tokio::time::{Instant, timeout_at};
use tokio_util::sync::CancellationToken;
use url::{Host, Url};
use uuid::Uuid;

use crate::credentials::AccessCredential;
use crate::error::{NativeClientError, retryable_pre_response_failure};

/// Default TCP/TLS connect bound for each connection attempt.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Default absolute deadline covering send, the single retry, and the body.
pub const DEFAULT_REQUEST_DEADLINE: Duration = Duration::from_secs(10);
/// Default ceiling for auth/session and operation response bodies.
pub const DEFAULT_JSON_BODY_LIMIT: usize = 64 * 1024;
/// Default ceiling for a container page response body.
pub const DEFAULT_COLLECTION_BODY_LIMIT: usize = 1024 * 1024;

/// Trust anchors used to validate the controller's TLS identity.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub enum TrustPolicy {
    /// Public WebPKI roots only.
    #[default]
    WebPki,
    /// A caller-supplied PEM bundle (`.pem`, `.crt`, or `.cer` containing one
    /// or more certificates) that replaces the public roots entirely.
    PemBundle(Vec<u8>),
}

/// Connection, deadline, and response-bound policy for a [`NativeClient`].
#[derive(Debug, Clone)]
pub struct NativeClientConfig {
    /// Controller origin: an `https` origin, or an explicit loopback `http`
    /// origin for development. No path, query, fragment, or userinfo.
    pub origin: String,
    /// TLS trust anchors.
    pub trust: TrustPolicy,
    /// Connect bound for each TCP/TLS connection attempt.
    pub connect_timeout: Duration,
    /// One absolute deadline per call, covering send, the single retry, and
    /// the response body.
    pub request_deadline: Duration,
    /// Ceiling for auth/session and operation response bodies.
    pub json_body_limit: usize,
    /// Ceiling for a container page response body.
    pub collection_body_limit: usize,
}

impl NativeClientConfig {
    /// Configuration for one controller origin with every default bound.
    #[must_use]
    pub fn new(origin: impl Into<String>) -> Self {
        Self {
            origin: origin.into(),
            ..Self::default()
        }
    }
}

impl Default for NativeClientConfig {
    fn default() -> Self {
        Self {
            origin: String::new(),
            trust: TrustPolicy::default(),
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            request_deadline: DEFAULT_REQUEST_DEADLINE,
            json_body_limit: DEFAULT_JSON_BODY_LIMIT,
            collection_body_limit: DEFAULT_COLLECTION_BODY_LIMIT,
        }
    }
}

/// UI-neutral controller client for the 0.1 API surface.
///
/// Construction validates the origin and the trust material without any
/// network traffic. The client is safe to share across calls; each call takes
/// its own cancellation token and credential arguments.
pub struct NativeClient {
    pub(crate) origin: Url,
    pub(crate) http: Client,
    pub(crate) limits: Limits,
}

/// Bounded-call policy copied out of the configuration.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    pub(crate) request_deadline: Duration,
    pub(crate) json_body_limit: usize,
    pub(crate) collection_body_limit: usize,
}

/// Whether a call may repeat once after a classified pre-response failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetryPolicy {
    Never,
    Once,
}

impl NativeClient {
    /// Validate configuration and build the client. No network traffic
    /// occurs, and an unreachable controller does not fail construction.
    pub fn new(config: NativeClientConfig) -> Result<Self, NativeClientError> {
        let origin = validate_origin(&config.origin)?;
        if config.connect_timeout.is_zero()
            || config.request_deadline.is_zero()
            || config.json_body_limit == 0
            || config.collection_body_limit == 0
        {
            return Err(NativeClientError::InvalidConfiguration);
        }
        let http = build_http_client(&config)?;
        Ok(Self {
            origin,
            http,
            limits: Limits {
                request_deadline: config.request_deadline,
                json_body_limit: config.json_body_limit,
                collection_body_limit: config.collection_body_limit,
            },
        })
    }

    /// Join a controller-absolute path onto the validated origin.
    pub(crate) fn endpoint(&self, path: &str) -> Result<Url, NativeClientError> {
        self.origin
            .join(path)
            .map_err(|_| NativeClientError::InvalidConfiguration)
    }

    /// Build a request that presents an access credential in exactly one
    /// sensitive `Authorization` header. The credential never enters the URL.
    pub(crate) fn authorized_request(
        &self,
        method: Method,
        url: Url,
        credential: &AccessCredential,
    ) -> Result<Request, NativeClientError> {
        let mut value = header::HeaderValue::from_str(&format!("Bearer {}", credential.expose()))
            .map_err(|_| NativeClientError::InvalidConfiguration)?;
        value.set_sensitive(true);
        self.http
            .request(method, url)
            .header(header::AUTHORIZATION, value)
            .header(header::ACCEPT, "application/json")
            .build()
            .map_err(|_| NativeClientError::InvalidConfiguration)
    }

    /// Execute one bounded exchange: single absolute deadline, explicit
    /// cancellation, at most one classified retry, and a streamed
    /// response-body ceiling. The success body is returned; a non-success
    /// status fails with its closed variant and the body is never read.
    pub(crate) async fn execute(
        &self,
        request: Request,
        policy: RetryPolicy,
        body_limit: usize,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, NativeClientError> {
        let deadline = Instant::now() + self.limits.request_deadline;
        tokio::select! {
            biased;
            () = cancellation.cancelled() => Err(NativeClientError::Cancelled),
            result = timeout_at(deadline, self.exchange(request, policy, body_limit)) => {
                match result {
                    Ok(result) => result,
                    Err(_) => Err(NativeClientError::DeadlineExceeded),
                }
            }
        }
    }

    async fn exchange(
        &self,
        request: Request,
        policy: RetryPolicy,
        body_limit: usize,
    ) -> Result<Vec<u8>, NativeClientError> {
        let retry_request = match policy {
            RetryPolicy::Never => None,
            RetryPolicy::Once => Some(
                request
                    .try_clone()
                    .ok_or(NativeClientError::InvalidConfiguration)?,
            ),
        };
        let response = match self.http.execute(request).await {
            Ok(response) => response,
            Err(error) => {
                let retry_request =
                    retry_request.filter(|_| retryable_pre_response_failure(&error));
                match retry_request {
                    Some(retry_request) => self
                        .http
                        .execute(retry_request)
                        .await
                        .map_err(|error| map_transport_error(&error))?,
                    None => return Err(map_transport_error(&error)),
                }
            }
        };
        finish(response, body_limit).await
    }
}

/// Validate an origin with a real URL parser. HTTPS is required unless the
/// host is `localhost` or a loopback IP literal over HTTP; `#77` confines
/// plaintext development to exactly those origins.
pub(crate) fn validate_origin(raw: &str) -> Result<Url, NativeClientError> {
    let url = Url::parse(raw).map_err(|_| NativeClientError::InvalidOrigin)?;
    match url.scheme() {
        "https" => {}
        "http" if host_is_loopback(&url) => {}
        _ => return Err(NativeClientError::InvalidOrigin),
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(NativeClientError::InvalidOrigin);
    }
    if url.query().is_some() || url.fragment().is_some() || url.path() != "/" {
        return Err(NativeClientError::InvalidOrigin);
    }
    Ok(url)
}

fn host_is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

fn build_http_client(config: &NativeClientConfig) -> Result<Client, NativeClientError> {
    let mut builder = Client::builder()
        .connect_timeout(config.connect_timeout)
        .redirect(redirect::Policy::none())
        .retry(retry::never())
        .referer(false)
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .no_proxy()
        .user_agent(concat!(
            "kontainer-native-client/",
            env!("CARGO_PKG_VERSION")
        ));
    if let TrustPolicy::PemBundle(pem) = &config.trust {
        let certificates = reqwest::Certificate::from_pem_bundle(pem)
            .map_err(|_| NativeClientError::InvalidConfiguration)?;
        if certificates.is_empty() {
            return Err(NativeClientError::InvalidConfiguration);
        }
        // An explicit bundle replaces the public roots: mixing operator
        // anchors into the public WebPKI set would silently widen trust.
        builder = builder.tls_built_in_root_certs(false);
        for certificate in certificates {
            builder = builder.add_root_certificate(certificate);
        }
    }
    builder
        .build()
        .map_err(|_| NativeClientError::InvalidConfiguration)
}

fn map_transport_error(error: &reqwest::Error) -> NativeClientError {
    if error.is_timeout() {
        NativeClientError::DeadlineExceeded
    } else {
        NativeClientError::Unavailable
    }
}

async fn finish(response: Response, body_limit: usize) -> Result<Vec<u8>, NativeClientError> {
    if !response.status().is_success() {
        return Err(NativeClientError::from_status(response.status()));
    }
    bounded_body(response, body_limit).await
}

/// Read a success body in chunks, rejecting anything above the ceiling before
/// it can be buffered whole.
async fn bounded_body(
    mut response: Response,
    body_limit: usize,
) -> Result<Vec<u8>, NativeClientError> {
    if response
        .content_length()
        .is_some_and(|length| length > body_limit as u64)
    {
        return Err(NativeClientError::InvalidResponse);
    }
    let mut buffer = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| NativeClientError::Unavailable)?
    {
        if buffer.len().saturating_add(chunk.len()) > body_limit {
            return Err(NativeClientError::InvalidResponse);
        }
        buffer.extend_from_slice(&chunk);
    }
    Ok(buffer)
}

/// Decode a bounded response body, mapping malformed input to a safe code.
pub(crate) fn decode_json<T: DeserializeOwned>(body: &[u8]) -> Result<T, NativeClientError> {
    serde_json::from_slice(body).map_err(|_| NativeClientError::InvalidResponse)
}

/// Parse a controller UUID string. Only the canonical hyphenated lowercase
/// form the controller emits is accepted; anything else is contract drift.
pub(crate) fn parse_uuid(value: &str) -> Result<Uuid, NativeClientError> {
    let parsed = Uuid::parse_str(value).map_err(|_| NativeClientError::InvalidResponse)?;
    if parsed.to_string() != value {
        return Err(NativeClientError::InvalidResponse);
    }
    Ok(parsed)
}

/// Parse a controller timestamp. The controller emits RFC 3339 UTC with
/// millisecond precision; any other rendering is contract drift.
pub(crate) fn parse_timestamp(
    value: &str,
) -> Result<chrono::DateTime<chrono::Utc>, NativeClientError> {
    use chrono::SecondsFormat;

    let parsed = chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .map_err(|_| NativeClientError::InvalidResponse)?;
    if parsed.to_rfc3339_opts(SecondsFormat::Millis, true) != value {
        return Err(NativeClientError::InvalidResponse);
    }
    Ok(parsed)
}
