use std::time::Duration;

pub const DEFAULT_MAXIMUM_FRAME_BYTES: usize = 64 * 1024;
pub const DEFAULT_MAXIMUM_QUEUED_FRAMES: usize = 32;
pub const DEFAULT_MAXIMUM_QUEUED_BYTES: usize = 1024 * 1024;
pub const DEFAULT_MAXIMUM_CONCURRENT_SESSIONS: usize = 4;
pub const DEFAULT_SHUTDOWN_GRACE: Duration = Duration::from_secs(2);
/// How long a client waits for the WebSocket upgrade to complete.
pub const DEFAULT_HANDSHAKE_DEADLINE: Duration = Duration::from_secs(15);
/// How often a server pings an open connection, so a quiet one still carries traffic.
pub const DEFAULT_PING_INTERVAL: Duration = Duration::from_secs(20);
/// How long a client waits for any message, Ping included, before it treats the connection as
/// gone; a little over two missed Pings.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(45);
const COMPILED_MAXIMUM_FRAME_BYTES: usize = 64 * 1024;
const COMPILED_MAXIMUM_QUEUED_FRAMES: usize = 1024;
const COMPILED_MAXIMUM_QUEUED_BYTES: usize = 16 * 1024 * 1024;
const COMPILED_MAXIMUM_CONCURRENT_SESSIONS: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamLimits {
    maximum_frame_bytes: usize,
    maximum_queued_frames: usize,
    maximum_queued_bytes: usize,
    maximum_concurrent_sessions: usize,
    shutdown_grace: Duration,
    handshake_deadline: Duration,
    ping_interval: Duration,
    idle_timeout: Duration,
}

impl StreamLimits {
    pub fn new(
        maximum_frame_bytes: usize,
        maximum_queued_frames: usize,
        maximum_queued_bytes: usize,
        maximum_concurrent_sessions: usize,
        shutdown_grace: Duration,
    ) -> Result<Self, StreamLimitError> {
        for (field, value) in [
            ("maximum_frame_bytes", maximum_frame_bytes),
            ("maximum_queued_frames", maximum_queued_frames),
            ("maximum_queued_bytes", maximum_queued_bytes),
            ("maximum_concurrent_sessions", maximum_concurrent_sessions),
        ] {
            if value == 0 {
                return Err(StreamLimitError::Zero { field });
            }
        }
        for (field, value, maximum) in [
            (
                "maximum_frame_bytes",
                maximum_frame_bytes,
                COMPILED_MAXIMUM_FRAME_BYTES,
            ),
            (
                "maximum_queued_frames",
                maximum_queued_frames,
                COMPILED_MAXIMUM_QUEUED_FRAMES,
            ),
            (
                "maximum_queued_bytes",
                maximum_queued_bytes,
                COMPILED_MAXIMUM_QUEUED_BYTES,
            ),
            (
                "maximum_concurrent_sessions",
                maximum_concurrent_sessions,
                COMPILED_MAXIMUM_CONCURRENT_SESSIONS,
            ),
        ] {
            if value > maximum {
                return Err(StreamLimitError::ExceedsCompiledMaximum { field, maximum });
            }
        }
        if shutdown_grace.is_zero() {
            return Err(StreamLimitError::Zero {
                field: "shutdown_grace",
            });
        }
        if std::time::Instant::now()
            .checked_add(shutdown_grace)
            .is_none()
        {
            return Err(StreamLimitError::ShutdownGraceOverflow);
        }

        Ok(Self {
            maximum_frame_bytes,
            maximum_queued_frames,
            maximum_queued_bytes,
            maximum_concurrent_sessions,
            shutdown_grace,
            ..Self::default()
        })
    }

    /// Replace the connection timers: the client's upgrade deadline, the server's Ping
    /// interval, and how long a client waits for any message.
    pub fn with_timers(
        self,
        handshake_deadline: Duration,
        ping_interval: Duration,
        idle_timeout: Duration,
    ) -> Result<Self, StreamLimitError> {
        for (field, value) in [
            ("handshake_deadline", handshake_deadline),
            ("ping_interval", ping_interval),
            ("idle_timeout", idle_timeout),
        ] {
            if value.is_zero() {
                return Err(StreamLimitError::Zero { field });
            }
        }
        Ok(Self {
            handshake_deadline,
            ping_interval,
            idle_timeout,
            ..self
        })
    }

    #[must_use]
    pub const fn maximum_frame_bytes(self) -> usize {
        self.maximum_frame_bytes
    }

    #[must_use]
    pub const fn maximum_queued_frames(self) -> usize {
        self.maximum_queued_frames
    }

    #[must_use]
    pub const fn maximum_queued_bytes(self) -> usize {
        self.maximum_queued_bytes
    }

    #[must_use]
    pub const fn maximum_concurrent_sessions(self) -> usize {
        self.maximum_concurrent_sessions
    }

    #[must_use]
    pub const fn shutdown_grace(self) -> Duration {
        self.shutdown_grace
    }

    #[must_use]
    pub const fn handshake_deadline(self) -> Duration {
        self.handshake_deadline
    }

    #[must_use]
    pub const fn ping_interval(self) -> Duration {
        self.ping_interval
    }

    #[must_use]
    pub const fn idle_timeout(self) -> Duration {
        self.idle_timeout
    }
}

impl Default for StreamLimits {
    fn default() -> Self {
        Self {
            maximum_frame_bytes: DEFAULT_MAXIMUM_FRAME_BYTES,
            maximum_queued_frames: DEFAULT_MAXIMUM_QUEUED_FRAMES,
            maximum_queued_bytes: DEFAULT_MAXIMUM_QUEUED_BYTES,
            maximum_concurrent_sessions: DEFAULT_MAXIMUM_CONCURRENT_SESSIONS,
            shutdown_grace: DEFAULT_SHUTDOWN_GRACE,
            handshake_deadline: DEFAULT_HANDSHAKE_DEADLINE,
            ping_interval: DEFAULT_PING_INTERVAL,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamLimitError {
    Zero { field: &'static str },
    ExceedsCompiledMaximum { field: &'static str, maximum: usize },
    ShutdownGraceOverflow,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        DEFAULT_HANDSHAKE_DEADLINE, DEFAULT_IDLE_TIMEOUT, DEFAULT_MAXIMUM_CONCURRENT_SESSIONS,
        DEFAULT_MAXIMUM_FRAME_BYTES, DEFAULT_MAXIMUM_QUEUED_BYTES, DEFAULT_MAXIMUM_QUEUED_FRAMES,
        DEFAULT_PING_INTERVAL, DEFAULT_SHUTDOWN_GRACE, StreamLimitError, StreamLimits,
    };

    #[test]
    fn public_defaults_are_explicit() {
        let limits = StreamLimits::default();

        assert_eq!(limits.maximum_frame_bytes(), DEFAULT_MAXIMUM_FRAME_BYTES);
        assert_eq!(
            limits.maximum_queued_frames(),
            DEFAULT_MAXIMUM_QUEUED_FRAMES
        );
        assert_eq!(limits.maximum_queued_bytes(), DEFAULT_MAXIMUM_QUEUED_BYTES);
        assert_eq!(
            limits.maximum_concurrent_sessions(),
            DEFAULT_MAXIMUM_CONCURRENT_SESSIONS
        );
        assert_eq!(limits.shutdown_grace(), DEFAULT_SHUTDOWN_GRACE);
        assert_eq!(limits.handshake_deadline(), DEFAULT_HANDSHAKE_DEADLINE);
        assert_eq!(limits.ping_interval(), DEFAULT_PING_INTERVAL);
        assert_eq!(limits.idle_timeout(), DEFAULT_IDLE_TIMEOUT);
        assert_eq!(
            StreamLimits::new(1, 1, 1, 1, Duration::from_secs(1))
                .unwrap()
                .idle_timeout(),
            DEFAULT_IDLE_TIMEOUT,
            "explicit sizes keep the default timers"
        );
    }

    #[test]
    fn timers_can_be_shortened_but_not_zeroed() {
        let second = Duration::from_secs(1);
        let limits = StreamLimits::default()
            .with_timers(second, 2 * second, 3 * second)
            .unwrap();
        assert_eq!(
            (
                limits.handshake_deadline(),
                limits.ping_interval(),
                limits.idle_timeout()
            ),
            (second, 2 * second, 3 * second)
        );
        assert_eq!(
            StreamLimits::default().with_timers(second, Duration::ZERO, second),
            Err(StreamLimitError::Zero {
                field: "ping_interval"
            })
        );
    }

    #[test]
    fn zero_and_pathological_limits_are_rejected() {
        assert_eq!(
            StreamLimits::new(1, 1, 1, 1, Duration::MAX),
            Err(StreamLimitError::ShutdownGraceOverflow)
        );
        assert_eq!(
            StreamLimits::new(0, 1, 1, 1, Duration::from_secs(1)),
            Err(StreamLimitError::Zero {
                field: "maximum_frame_bytes"
            })
        );
        assert_eq!(
            StreamLimits::new(
                1,
                1,
                super::COMPILED_MAXIMUM_QUEUED_BYTES + 1,
                1,
                Duration::from_secs(1)
            ),
            Err(StreamLimitError::ExceedsCompiledMaximum {
                field: "maximum_queued_bytes",
                maximum: super::COMPILED_MAXIMUM_QUEUED_BYTES,
            })
        );
        assert_eq!(
            StreamLimits::new(
                super::COMPILED_MAXIMUM_FRAME_BYTES + 1,
                1,
                1,
                1,
                Duration::from_secs(1)
            ),
            Err(StreamLimitError::ExceedsCompiledMaximum {
                field: "maximum_frame_bytes",
                maximum: super::COMPILED_MAXIMUM_FRAME_BYTES,
            })
        );
        assert_eq!(
            StreamLimits::new(
                1,
                super::COMPILED_MAXIMUM_QUEUED_FRAMES + 1,
                1,
                1,
                Duration::from_secs(1)
            ),
            Err(StreamLimitError::ExceedsCompiledMaximum {
                field: "maximum_queued_frames",
                maximum: super::COMPILED_MAXIMUM_QUEUED_FRAMES,
            })
        );
        assert_eq!(
            StreamLimits::new(
                1,
                1,
                1,
                super::COMPILED_MAXIMUM_CONCURRENT_SESSIONS + 1,
                Duration::from_secs(1)
            ),
            Err(StreamLimitError::ExceedsCompiledMaximum {
                field: "maximum_concurrent_sessions",
                maximum: super::COMPILED_MAXIMUM_CONCURRENT_SESSIONS,
            })
        );
    }
}
