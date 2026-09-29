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
    /// The daemon has no such object (404).
    NotFound,
    /// Already in the requested state (304).
    NotModified,
    /// The object's state forbids the request, e.g. removing a running container (409).
    Conflict,
    /// The Swarm has no leader: too few managers are reachable to change or read cluster state.
    QuorumUnavailable,
    /// The object changed since the version the request was based on.
    VersionConflict,
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
    ReadContainerStats,
    ReadDiskUsage,
    InspectContainer,
    ContainerAction,
    ListImages,
    RemoveImage,
    PullImage,
    ListVolumes,
    CreateVolume,
    RemoveVolume,
    ListNetworks,
    InspectNetwork,
    CreateNetwork,
    RemoveNetwork,
    ListServices,
    ListTasks,
    UpdateService,
    ReadSwarm,
    ReadServiceLogs,
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
            Self::ReadContainerStats => "read container stats",
            Self::ReadDiskUsage => "read disk usage",
            Self::InspectContainer => "inspect container",
            Self::ContainerAction => "container action",
            Self::ListImages => "list images",
            Self::RemoveImage => "remove image",
            Self::PullImage => "pull image",
            Self::ListVolumes => "list volumes",
            Self::CreateVolume => "create volume",
            Self::RemoveVolume => "remove volume",
            Self::ListNetworks => "list networks",
            Self::InspectNetwork => "inspect network",
            Self::CreateNetwork => "create network",
            Self::RemoveNetwork => "remove network",
            Self::ListServices => "list services",
            Self::ListTasks => "list tasks",
            Self::UpdateService => "update service",
            Self::ReadSwarm => "read swarm state",
            Self::ReadServiceLogs => "read service logs",
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
        // A removed `before` cursor surfaces as a daemon 404. For inventory
        // that means stale pagination, not an unavailable Engine.
        if operation == EngineOperation::ListContainers
            && matches!(
                error,
                BollardError::DockerResponseServerError {
                    status_code: 404,
                    ..
                }
            )
        {
            return Self::new(EngineErrorKind::InvalidInput, operation);
        }
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
        BollardError::DockerResponseServerError {
            status_code: 404, ..
        } => EngineErrorKind::NotFound,
        BollardError::DockerResponseServerError {
            status_code: 304, ..
        } => EngineErrorKind::NotModified,
        BollardError::DockerResponseServerError { message, .. } if is_quorum_message(message) => {
            EngineErrorKind::QuorumUnavailable
        }
        BollardError::DockerResponseServerError { message, .. }
            if message.contains("update out of sequence") =>
        {
            EngineErrorKind::VersionConflict
        }
        BollardError::DockerResponseServerError {
            status_code: 409, ..
        } => EngineErrorKind::Conflict,
        BollardError::DockerResponseServerError { .. } => EngineErrorKind::DaemonRejected,
        BollardError::IOError { err } => classify_io_error(err),
        BollardError::HyperResponseError { .. } | BollardError::HyperLegacyError { .. } => {
            EngineErrorKind::ConnectionUnavailable
        }
        _ => EngineErrorKind::DependencyFailure,
    }
}

/// Swarm's own wording when a manager can't reach a majority. The text only classifies the error;
/// it never leaves the engine.
fn is_quorum_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("does not have a leader")
        || lower.contains("quorum")
        || lower.contains("no leader")
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
    fn swarm_quorum_and_version_messages_map_to_their_own_kinds() {
        for message in [
            "rpc error: code = Unknown desc = The swarm does not have a leader. It's possible that too few managers are online.",
            "context deadline exceeded: raft quorum lost",
        ] {
            let error = BollardError::DockerResponseServerError {
                status_code: 503,
                message: message.to_owned(),
            };
            assert_eq!(
                EngineError::from_bollard(EngineOperation::ListServices, &error).kind(),
                EngineErrorKind::QuorumUnavailable,
                "{message}"
            );
        }
        let out_of_sequence = BollardError::DockerResponseServerError {
            status_code: 500,
            message: "rpc error: code = Unknown desc = update out of sequence".to_owned(),
        };
        assert_eq!(
            EngineError::from_bollard(EngineOperation::UpdateService, &out_of_sequence).kind(),
            EngineErrorKind::VersionConflict
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
    fn removed_cursor_is_invalid_pagination_not_unavailable() {
        let not_found = BollardError::DockerResponseServerError {
            status_code: 404,
            message: "No such container".to_owned(),
        };
        let error = EngineError::from_bollard(EngineOperation::ListContainers, &not_found);
        assert_eq!(error.kind(), EngineErrorKind::InvalidInput);

        // Elsewhere a 404 means the object is gone.
        let error = EngineError::from_bollard(EngineOperation::ReadVersion, &not_found);
        assert_eq!(error.kind(), EngineErrorKind::NotFound);
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
