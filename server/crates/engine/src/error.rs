use std::{error::Error, fmt, io};

use bollard::errors::Error as BollardError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineErrorKind {
    InvalidConfiguration,
    InvalidInput,
    ConnectionUnavailable,
    DeadlineExceeded,
    Cancelled,
    InvalidResponse,
    DaemonRejected,
    UnsupportedApiVersion,
    UnsupportedCapability,
    DependencyFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineOperation {
    Connect,
    NegotiateApiVersion,
    Ping,
    ReadVersion,
    ReadSystemInfo,
    Describe,
    ListContainers,
}

impl fmt::Display for EngineOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Connect => "connect",
            Self::NegotiateApiVersion => "negotiate API version",
            Self::Ping => "ping",
            Self::ReadVersion => "read version",
            Self::ReadSystemInfo => "read system information",
            Self::Describe => "describe",
            Self::ListContainers => "list containers",
        };
        formatter.write_str(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError {
    kind: EngineErrorKind,
    operation: EngineOperation,
}

impl EngineError {
    #[must_use]
    pub const fn new(kind: EngineErrorKind, operation: EngineOperation) -> Self {
        Self { kind, operation }
    }

    #[must_use]
    pub const fn kind(&self) -> EngineErrorKind {
        self.kind
    }

    #[must_use]
    pub const fn operation(&self) -> EngineOperation {
        self.operation
    }

    pub(crate) fn from_bollard(operation: EngineOperation, error: &BollardError) -> Self {
        Self::new(classify_bollard_error(error), operation)
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Docker Engine {} failed ({:?})",
            self.operation, self.kind
        )
    }
}

impl Error for EngineError {}

fn classify_bollard_error(error: &BollardError) -> EngineErrorKind {
    match error {
        BollardError::SocketNotFoundError(_) => EngineErrorKind::ConnectionUnavailable,
        BollardError::RequestTimeoutError => EngineErrorKind::DeadlineExceeded,
        BollardError::APIVersionParseError {} | BollardError::JsonDataError { .. } => {
            EngineErrorKind::InvalidResponse
        }
        BollardError::DockerResponseServerError { .. } => EngineErrorKind::DaemonRejected,
        BollardError::IOError { err } => classify_io_error(err),
        BollardError::HyperResponseError { .. } | BollardError::HyperLegacyError { .. } => {
            EngineErrorKind::ConnectionUnavailable
        }
        _ => EngineErrorKind::DependencyFailure,
    }
}

fn classify_io_error(error: &io::Error) -> EngineErrorKind {
    match error.kind() {
        io::ErrorKind::NotFound
        | io::ErrorKind::ConnectionRefused
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::NotConnected
        | io::ErrorKind::BrokenPipe
        | io::ErrorKind::UnexpectedEof => EngineErrorKind::ConnectionUnavailable,
        io::ErrorKind::TimedOut => EngineErrorKind::DeadlineExceeded,
        io::ErrorKind::InvalidData => EngineErrorKind::InvalidResponse,
        _ => EngineErrorKind::DependencyFailure,
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use bollard::errors::Error as BollardError;

    use super::{EngineError, EngineErrorKind, EngineOperation};

    #[test]
    fn connection_refusal_maps_to_a_stable_kind() {
        let error = BollardError::IOError {
            err: io::Error::from(io::ErrorKind::ConnectionRefused),
        };

        assert_eq!(
            EngineError::from_bollard(EngineOperation::Ping, &error).kind(),
            EngineErrorKind::ConnectionUnavailable
        );
    }

    #[test]
    fn daemon_shutdown_maps_to_a_stable_kind() {
        let error = BollardError::IOError {
            err: io::Error::from(io::ErrorKind::ConnectionReset),
        };

        assert_eq!(
            EngineError::from_bollard(EngineOperation::ReadSystemInfo, &error).kind(),
            EngineErrorKind::ConnectionUnavailable
        );
    }

    #[test]
    fn malformed_and_daemon_error_responses_are_distinct() {
        let malformed = BollardError::JsonDataError {
            message: "redacted fixture".to_owned(),
            column: 1,
        };
        let rejected = BollardError::DockerResponseServerError {
            status_code: 503,
            message: "redacted fixture".to_owned(),
        };

        assert_eq!(
            EngineError::from_bollard(EngineOperation::ReadVersion, &malformed).kind(),
            EngineErrorKind::InvalidResponse
        );
        assert_eq!(
            EngineError::from_bollard(EngineOperation::ReadVersion, &rejected).kind(),
            EngineErrorKind::DaemonRejected
        );
    }

    #[test]
    fn public_error_text_does_not_include_daemon_details() {
        let error = BollardError::DockerResponseServerError {
            status_code: 500,
            message: "secret daemon path".to_owned(),
        };
        let mapped = EngineError::from_bollard(EngineOperation::ReadVersion, &error);

        assert!(!mapped.to_string().contains("secret daemon path"));
        assert_eq!(mapped.kind(), EngineErrorKind::DaemonRejected);
    }
}
