//! Closed, body-free client error taxonomy.
//!
//! Every variant is a stable, safe code. No variant carries a response body,
//! transport message, URL, header, or credential. [`NativeClientError::UnexpectedStatus`]
//! exposes only the numeric status, which the peer already observed on the
//! wire.

use std::io::ErrorKind;

use reqwest::StatusCode;

/// Every failure a native client call can report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum NativeClientError {
    /// The configured controller origin is not a usable origin.
    #[error("controller origin is not valid")]
    InvalidOrigin,
    /// Client configuration or trust material is not usable.
    #[error("client configuration is not valid")]
    InvalidConfiguration,
    /// The node ID is not a canonical controller node identifier.
    #[error("node ID is not valid")]
    InvalidNodeId,
    /// The page size or cursor is outside the documented bounds.
    #[error("pagination is not valid")]
    InvalidPagination,
    /// The caller's cancellation token fired before the call completed.
    #[error("request was cancelled")]
    Cancelled,
    /// The call's single absolute deadline expired.
    #[error("request deadline expired")]
    DeadlineExceeded,
    /// The controller could not be reached, or the exchange did not complete.
    #[error("controller is unreachable")]
    Unavailable,
    /// The controller response violated the documented contract.
    #[error("controller response is not valid")]
    InvalidResponse,
    /// The controller rejected the request as malformed (`400`).
    #[error("request was rejected")]
    InvalidRequest,
    /// Authentication failed (`401`).
    #[error("authentication failed")]
    Unauthorized,
    /// The controller denied an authenticated request (`403`).
    #[error("request is forbidden")]
    Forbidden,
    /// The resource is absent or not visible to the caller (`404`).
    #[error("resource was not found")]
    NotFound,
    /// The resource or login attempt is not ready yet (`409`).
    #[error("resource is not ready")]
    Conflict,
    /// GitHub was unreachable or rejected the exchange (`502`).
    #[error("identity provider is unavailable")]
    ProviderUnavailable,
    /// The controller or one of its dependencies is unavailable (`503`).
    #[error("controller is not available")]
    ServiceUnavailable,
    /// A controller dependency exceeded its deadline (`504`).
    #[error("controller dependency timed out")]
    GatewayTimeout,
    /// Any other non-success status, with no body attached.
    #[error("controller returned status {status}")]
    UnexpectedStatus {
        /// Numeric HTTP status observed on the wire.
        status: u16,
    },
}

impl NativeClientError {
    /// Map a non-success HTTP status to its closed variant.
    pub(crate) fn from_status(status: StatusCode) -> Self {
        match status.as_u16() {
            400 => Self::InvalidRequest,
            401 => Self::Unauthorized,
            403 => Self::Forbidden,
            404 => Self::NotFound,
            409 => Self::Conflict,
            502 => Self::ProviderUnavailable,
            503 => Self::ServiceUnavailable,
            504 => Self::GatewayTimeout,
            status => Self::UnexpectedStatus { status },
        }
    }
}

/// Classify a transport failure as safe to retry exactly once.
///
/// The retry set is deliberately tiny: a positively identified transient I/O
/// cause (connection reset, refused, aborted, broken pipe, unexpected EOF, or
/// not connected). Certificate, identity, and hostname failures do not carry
/// an allowlisted cause, and unknown or ambiguous causes fail closed.
pub(crate) fn retryable_pre_response_failure(error: &reqwest::Error) -> bool {
    if error.is_timeout() || error.is_body() || error.is_decode() || error.is_redirect() {
        return false;
    }
    std::error::Error::source(error).is_some_and(retryable_cause_chain)
}

/// Walk a transport cause chain for an allowlisted I/O cause.
pub(crate) fn retryable_cause_chain(source: &(dyn std::error::Error + 'static)) -> bool {
    let mut current = source;
    let mut allowlisted = false;
    loop {
        if let Some(io) = current.downcast_ref::<std::io::Error>()
            && matches!(
                io.kind(),
                ErrorKind::ConnectionReset
                    | ErrorKind::ConnectionRefused
                    | ErrorKind::ConnectionAborted
                    | ErrorKind::BrokenPipe
                    | ErrorKind::UnexpectedEof
                    | ErrorKind::NotConnected
            )
        {
            allowlisted = true;
        }
        match std::error::Error::source(current) {
            Some(next) => current = next,
            None => return allowlisted,
        }
    }
}
