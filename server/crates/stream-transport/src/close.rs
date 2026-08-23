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
    pub const BINARY_REQUIRED: Self = Self::new(1003, "binary protobuf required");
    pub const MALFORMED_ENVELOPE: Self = Self::new(1007, "malformed protobuf envelope");
    pub const MESSAGE_TOO_LARGE: Self = Self::new(1009, "frame exceeds configured limit");
    pub const APPLICATION_FAILURE: Self = Self::new(1011, "stream application failed");
    pub const SLOW_CONSUMER: Self = Self::new(1013, "slow consumer exceeded queue limits");

    #[must_use]
    pub const fn new(code: u16, reason: &'static str) -> Self {
        Self { code, reason }
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
            StreamClose::BINARY_REQUIRED,
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
