//! Log stream payloads: one message per frame kind, carried in the envelope's `payload`.

use prost::Message;

use crate::{StreamFrame, StreamFrameKind, wire};

/// Lines sent before following when `LogsOpen.tail` is absent.
pub const DEFAULT_LOG_TAIL: u32 = 500;
/// The most history one open may ask for.
pub const MAX_LOG_TAIL: u32 = 5_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogsOpen {
    pub service_id: String,
    pub since_unix_ms: Option<i64>,
    pub tail: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStreamKind {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub seq: u64,
    pub ts_unix_ms: i64,
    pub stream: LogStreamKind,
    pub task_id: String,
    pub node_id: String,
    pub line: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogGap {
    pub dropped: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogEndReason {
    ServiceRemoved,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEnd {
    pub reason: LogEndReason,
    pub code: String,
    pub message: String,
}

/// Every log payload, in either direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogMessage {
    Open(LogsOpen),
    Close,
    Line(LogLine),
    Gap(LogGap),
    End(LogEnd),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogPayloadError {
    /// The frame kind carries no log payload.
    NotALogFrame,
    Malformed,
    /// An enum field was unspecified or unknown.
    UnknownValue,
}

impl LogMessage {
    #[must_use]
    pub fn into_frame(self, sequence: u64) -> StreamFrame {
        let (kind, payload) = match self {
            Self::Open(open) => (
                StreamFrameKind::LogsOpen,
                wire::LogsOpen {
                    service_id: open.service_id,
                    since_unix_ms: open.since_unix_ms,
                    tail: open.tail,
                }
                .encode_to_vec(),
            ),
            Self::Close => (
                StreamFrameKind::LogsClose,
                wire::LogsClose {}.encode_to_vec(),
            ),
            Self::Line(line) => (
                StreamFrameKind::LogLine,
                wire::LogLine {
                    seq: line.seq,
                    ts_unix_ms: line.ts_unix_ms,
                    stream: match line.stream {
                        LogStreamKind::Stdout => wire::LogStreamKind::Stdout,
                        LogStreamKind::Stderr => wire::LogStreamKind::Stderr,
                    }
                    .into(),
                    task_id: line.task_id,
                    node_id: line.node_id,
                    line: line.line,
                    truncated: line.truncated,
                }
                .encode_to_vec(),
            ),
            Self::Gap(gap) => (
                StreamFrameKind::LogGap,
                wire::LogGap {
                    dropped: gap.dropped,
                }
                .encode_to_vec(),
            ),
            Self::End(end) => (
                StreamFrameKind::LogEnd,
                wire::LogEnd {
                    reason: match end.reason {
                        LogEndReason::ServiceRemoved => wire::LogEndReason::ServiceRemoved,
                        LogEndReason::Error => wire::LogEndReason::Error,
                    }
                    .into(),
                    code: end.code,
                    message: end.message,
                }
                .encode_to_vec(),
            ),
        };
        StreamFrame {
            sequence,
            kind,
            payload,
        }
    }

    pub fn from_frame(frame: &StreamFrame) -> Result<Self, LogPayloadError> {
        let bytes = frame.payload.as_slice();
        let malformed = |_| LogPayloadError::Malformed;
        Ok(match frame.kind {
            StreamFrameKind::LogsOpen => {
                let open = wire::LogsOpen::decode(bytes).map_err(malformed)?;
                Self::Open(LogsOpen {
                    service_id: open.service_id,
                    since_unix_ms: open.since_unix_ms,
                    tail: open.tail,
                })
            }
            StreamFrameKind::LogsClose => {
                wire::LogsClose::decode(bytes).map_err(malformed)?;
                Self::Close
            }
            StreamFrameKind::LogLine => {
                let line = wire::LogLine::decode(bytes).map_err(malformed)?;
                let stream = match wire::LogStreamKind::try_from(line.stream) {
                    Ok(wire::LogStreamKind::Stdout) => LogStreamKind::Stdout,
                    Ok(wire::LogStreamKind::Stderr) => LogStreamKind::Stderr,
                    _ => return Err(LogPayloadError::UnknownValue),
                };
                Self::Line(LogLine {
                    seq: line.seq,
                    ts_unix_ms: line.ts_unix_ms,
                    stream,
                    task_id: line.task_id,
                    node_id: line.node_id,
                    line: line.line,
                    truncated: line.truncated,
                })
            }
            StreamFrameKind::LogGap => {
                let gap = wire::LogGap::decode(bytes).map_err(malformed)?;
                Self::Gap(LogGap {
                    dropped: gap.dropped,
                })
            }
            StreamFrameKind::LogEnd => {
                let end = wire::LogEnd::decode(bytes).map_err(malformed)?;
                let reason = match wire::LogEndReason::try_from(end.reason) {
                    Ok(wire::LogEndReason::ServiceRemoved) => LogEndReason::ServiceRemoved,
                    Ok(wire::LogEndReason::Error) => LogEndReason::Error,
                    _ => return Err(LogPayloadError::UnknownValue),
                };
                Self::End(LogEnd {
                    reason,
                    code: end.code,
                    message: end.message,
                })
            }
            StreamFrameKind::Data | StreamFrameKind::Acknowledgement => {
                return Err(LogPayloadError::NotALogFrame);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LogEnd, LogEndReason, LogGap, LogLine, LogMessage, LogPayloadError, LogStreamKind, LogsOpen,
    };
    use crate::{StreamFrame, StreamFrameKind};

    #[test]
    fn every_log_message_round_trips_through_an_envelope() {
        let messages = [
            LogMessage::Open(LogsOpen {
                service_id: "eb6ymskrt4ij70dco3menddxl".to_owned(),
                since_unix_ms: Some(1_790_000_000_000),
                tail: Some(500),
            }),
            LogMessage::Open(LogsOpen {
                service_id: "eb6ymskrt4ij70dco3menddxl".to_owned(),
                since_unix_ms: None,
                tail: None,
            }),
            LogMessage::Close,
            LogMessage::Line(LogLine {
                seq: 7,
                ts_unix_ms: 1_790_000_000_123,
                stream: LogStreamKind::Stderr,
                task_id: "x0xza695s4xgf5rqmbvkpsizf".to_owned(),
                node_id: "tj42rq5nzwf8r8avruy1a45b6".to_owned(),
                line: "GET / 200".to_owned(),
                truncated: true,
            }),
            LogMessage::Gap(LogGap { dropped: 42 }),
            LogMessage::End(LogEnd {
                reason: LogEndReason::ServiceRemoved,
                code: "service_removed".to_owned(),
                message: "The service was removed.".to_owned(),
            }),
        ];
        for (sequence, message) in messages.into_iter().enumerate() {
            let frame = message.clone().into_frame(sequence as u64);
            let decoded = StreamFrame::decode(&frame.encode()).unwrap();
            assert_eq!(decoded.sequence, sequence as u64);
            assert_eq!(LogMessage::from_frame(&decoded).unwrap(), message);
        }
    }

    #[test]
    fn non_log_frames_and_unknown_values_are_refused() {
        assert_eq!(
            LogMessage::from_frame(&StreamFrame::data(1, b"ping".to_vec())),
            Err(LogPayloadError::NotALogFrame)
        );
        let unspecified_stream = StreamFrame {
            sequence: 1,
            kind: StreamFrameKind::LogLine,
            payload: Vec::new(),
        };
        assert_eq!(
            LogMessage::from_frame(&unspecified_stream),
            Err(LogPayloadError::UnknownValue)
        );
        let garbage = StreamFrame {
            sequence: 1,
            kind: StreamFrameKind::LogsOpen,
            payload: vec![0xff, 0xff],
        };
        assert_eq!(
            LogMessage::from_frame(&garbage),
            Err(LogPayloadError::Malformed)
        );
    }
}
