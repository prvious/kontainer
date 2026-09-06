use kontainer_stream_protocol::StreamProtocolError;
use tokio_tungstenite::tungstenite::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamClose {
    pub code: u16,
    pub reason: &'static str,
}

impl StreamClose {
    pub const NORMAL: Self = Self::new(1000, "stream complete");
    pub const SERVER_SHUTDOWN: Self = Self::new(1001, "server shutdown");
    pub const CLIENT_CANCELLED: Self = Self::new(1001, "client cancelled");
    pub const UNSUPPORTED_VERSION: Self = Self::new(1002, "unsupported protocol version");
    pub const UNKNOWN_FRAME_KIND: Self = Self::new(1002, "unknown frame kind");
    pub const PROTOCOL_VIOLATION: Self = Self::new(1002, "websocket protocol violation");
    pub const BINARY_REQUIRED: Self = Self::new(1003, "binary protobuf required");
    pub const INVALID_UTF8: Self = Self::new(1007, "invalid websocket utf-8");
    pub const MALFORMED_ENVELOPE: Self = Self::new(1007, "malformed protobuf envelope");
    pub const MESSAGE_TOO_LARGE: Self = Self::new(1009, "frame exceeds configured limit");
    pub const APPLICATION_FAILURE: Self = Self::new(1011, "stream application failed");
    pub const SLOW_CONSUMER: Self = Self::new(1013, "slow consumer exceeded queue limits");

    #[must_use]
    pub const fn new(code: u16, reason: &'static str) -> Self {
        Self { code, reason }
    }

    pub(crate) fn for_envelope_error(error: StreamProtocolError) -> Self {
        match error {
            StreamProtocolError::MalformedEnvelope => Self::MALFORMED_ENVELOPE,
            StreamProtocolError::UnsupportedProtocolVersion(_) => Self::UNSUPPORTED_VERSION,
            StreamProtocolError::UnknownFrameKind(_) => Self::UNKNOWN_FRAME_KIND,
        }
    }

    pub(crate) fn for_transport_error(error: &Error) -> Option<Self> {
        match error {
            Error::Capacity(_) => Some(Self::MESSAGE_TOO_LARGE),
            Error::Protocol(_) => Some(Self::PROTOCOL_VIOLATION),
            Error::Utf8(_) => Some(Self::INVALID_UTF8),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StreamClose;

    #[test]
    fn owned_close_reasons_fit_the_websocket_control_frame_limit() {
        for close in [
            StreamClose::NORMAL,
            StreamClose::SERVER_SHUTDOWN,
            StreamClose::CLIENT_CANCELLED,
            StreamClose::UNSUPPORTED_VERSION,
            StreamClose::UNKNOWN_FRAME_KIND,
            StreamClose::PROTOCOL_VIOLATION,
            StreamClose::BINARY_REQUIRED,
            StreamClose::INVALID_UTF8,
            StreamClose::MALFORMED_ENVELOPE,
            StreamClose::MESSAGE_TOO_LARGE,
            StreamClose::APPLICATION_FAILURE,
            StreamClose::SLOW_CONSUMER,
        ] {
            assert!(close.reason.len() <= 123);
            assert!(!close.reason.is_empty());
        }
    }
}
