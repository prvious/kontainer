//! `GET /health/ready` as a reachability probe with a failure class.

use std::error::Error as StdError;

use reqwest::header;
use tokio::time::{Instant, timeout_at};
use tokio_util::sync::CancellationToken;

use crate::client::NativeClient;

/// Where a probe stopped, in the order a connection is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeFailure {
    /// The host name did not resolve.
    Dns,
    /// Nothing accepted the TCP connection, or it dropped.
    Tcp,
    /// The TLS handshake or certificate check failed.
    Tls,
    /// The controller answered, but not ready (non-2xx).
    Http,
    /// No answer within the client's request deadline.
    Timeout,
}

impl NativeClient {
    /// Probe the controller's readiness endpoint once under the configured
    /// deadline. `Ok(())` means `/health/ready` answered 2xx.
    ///
    /// # Errors
    ///
    /// A [`ProbeFailure`] naming the step that failed.
    pub async fn probe_ready(&self, cancellation: &CancellationToken) -> Result<(), ProbeFailure> {
        let url = self
            .endpoint("/health/ready")
            .map_err(|_| ProbeFailure::Tcp)?;
        let request = self
            .http
            .get(url)
            .header(header::ACCEPT, "application/json")
            .build()
            .map_err(|_| ProbeFailure::Tcp)?;
        let deadline = Instant::now()
            .checked_add(self.limits.request_deadline)
            .ok_or(ProbeFailure::Timeout)?;
        let response = tokio::select! {
            biased;
            () = cancellation.cancelled() => return Err(ProbeFailure::Timeout),
            result = timeout_at(deadline, self.http.execute(request)) => match result {
                Err(_) => return Err(ProbeFailure::Timeout),
                Ok(Err(error)) => return Err(classify(&error)),
                Ok(Ok(response)) => response,
            },
        };
        if response.status().is_success() {
            Ok(())
        } else {
            Err(ProbeFailure::Http)
        }
    }
}

/// Classify a transport failure by walking its cause chain. DNS failures
/// carry no dedicated type, so they are recognized by the resolver's text.
fn classify(error: &reqwest::Error) -> ProbeFailure {
    if error.is_timeout() {
        return ProbeFailure::Timeout;
    }
    let mut current: Option<&(dyn StdError + 'static)> = Some(error);
    while let Some(cause) = current {
        if cause.downcast_ref::<rustls::Error>().is_some() {
            return ProbeFailure::Tls;
        }
        let text = cause.to_string().to_ascii_lowercase();
        if text.contains("dns error")
            || text.contains("failed to lookup address")
            || text.contains("nodename nor servname")
            || text.contains("name or service not known")
        {
            return ProbeFailure::Dns;
        }
        if text.contains("certificate") || text.contains("handshake") || text.contains("tls") {
            return ProbeFailure::Tls;
        }
        current = cause.source();
    }
    ProbeFailure::Tcp
}
