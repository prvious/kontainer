#![forbid(unsafe_code)]

use prost::Message;

pub const PROTOCOL_VERSION: u32 = 1;
pub const WEBSOCKET_SUBPROTOCOL: &str = "kontainer.stream.v1";

mod exec;
mod logs;

pub use exec::{
    ExecError, ExecExit, ExecMessage, ExecOpen, ExecPayloadError, ExecResize, ExecStarted,
    MAX_EXEC_DIMENSION,
};
pub use logs::{
    DEFAULT_LOG_TAIL, LogEnd, LogEndReason, LogGap, LogLine, LogMessage, LogPayloadError,
    LogStreamKind, LogsOpen, MAX_LOG_TAIL,
};

mod wire {
    include!(concat!(env!("OUT_DIR"), "/kontainer.stream.v1.rs"));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamFrameKind {
    Data,
    Acknowledgement,
    LogsOpen,
    LogsClose,
    LogLine,
    LogGap,
    LogEnd,
    ExecOpen,
    ExecInput,
    ExecResize,
    ExecClose,
    ExecStarted,
    ExecOutput,
    ExecExit,
    ExecError,
}

impl StreamFrameKind {
    fn to_wire(self) -> wire::FrameKind {
        match self {
            Self::Data => wire::FrameKind::Data,
            Self::Acknowledgement => wire::FrameKind::Acknowledgement,
            Self::LogsOpen => wire::FrameKind::LogsOpen,
            Self::LogsClose => wire::FrameKind::LogsClose,
            Self::LogLine => wire::FrameKind::LogLine,
            Self::LogGap => wire::FrameKind::LogGap,
            Self::LogEnd => wire::FrameKind::LogEnd,
            Self::ExecOpen => wire::FrameKind::ExecOpen,
            Self::ExecInput => wire::FrameKind::ExecInput,
            Self::ExecResize => wire::FrameKind::ExecResize,
            Self::ExecClose => wire::FrameKind::ExecClose,
            Self::ExecStarted => wire::FrameKind::ExecStarted,
            Self::ExecOutput => wire::FrameKind::ExecOutput,
            Self::ExecExit => wire::FrameKind::ExecExit,
            Self::ExecError => wire::FrameKind::ExecError,
        }
    }

    fn from_wire(kind: wire::FrameKind) -> Option<Self> {
        Some(match kind {
            wire::FrameKind::Unspecified => return None,
            wire::FrameKind::Data => Self::Data,
            wire::FrameKind::Acknowledgement => Self::Acknowledgement,
            wire::FrameKind::LogsOpen => Self::LogsOpen,
            wire::FrameKind::LogsClose => Self::LogsClose,
            wire::FrameKind::LogLine => Self::LogLine,
            wire::FrameKind::LogGap => Self::LogGap,
            wire::FrameKind::LogEnd => Self::LogEnd,
            wire::FrameKind::ExecOpen => Self::ExecOpen,
            wire::FrameKind::ExecInput => Self::ExecInput,
            wire::FrameKind::ExecResize => Self::ExecResize,
            wire::FrameKind::ExecClose => Self::ExecClose,
            wire::FrameKind::ExecStarted => Self::ExecStarted,
            wire::FrameKind::ExecOutput => Self::ExecOutput,
            wire::FrameKind::ExecExit => Self::ExecExit,
            wire::FrameKind::ExecError => Self::ExecError,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamFrame {
    pub sequence: u64,
    pub kind: StreamFrameKind,
    pub payload: Vec<u8>,
}

impl StreamFrame {
    #[must_use]
    pub fn data(sequence: u64, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            sequence,
            kind: StreamFrameKind::Data,
            payload: payload.into(),
        }
    }

    #[must_use]
    pub fn acknowledgement(sequence: u64, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            sequence,
            kind: StreamFrameKind::Acknowledgement,
            payload: payload.into(),
        }
    }

    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        wire::StreamEnvelope {
            protocol_version: PROTOCOL_VERSION,
            sequence: self.sequence,
            kind: self.kind.to_wire().into(),
            payload: self.payload.clone(),
        }
        .encode_to_vec()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, StreamProtocolError> {
        let envelope = wire::StreamEnvelope::decode(bytes)
            .map_err(|_| StreamProtocolError::MalformedEnvelope)?;

        if envelope.protocol_version != PROTOCOL_VERSION {
            return Err(StreamProtocolError::UnsupportedProtocolVersion(
                envelope.protocol_version,
            ));
        }

        let kind = wire::FrameKind::try_from(envelope.kind)
            .ok()
            .and_then(StreamFrameKind::from_wire)
            .ok_or(StreamProtocolError::UnknownFrameKind(envelope.kind))?;

        Ok(Self {
            sequence: envelope.sequence,
            kind,
            payload: envelope.payload,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamProtocolError {
    MalformedEnvelope,
    UnsupportedProtocolVersion(u32),
    UnknownFrameKind(i32),
}

#[cfg(test)]
mod tests {
    use super::{PROTOCOL_VERSION, StreamFrame, StreamFrameKind, StreamProtocolError, wire};
    use prost::Message;

    const DATA_FRAME_FIXTURE: &[u8] = &[8, 1, 16, 7, 24, 1, 34, 4, 112, 105, 110, 103];

    #[test]
    fn binary_fixture_is_stable_and_round_trips() {
        let frame = StreamFrame::data(7, b"ping".to_vec());

        assert_eq!(frame.encode(), DATA_FRAME_FIXTURE);
        assert_eq!(StreamFrame::decode(DATA_FRAME_FIXTURE).unwrap(), frame);
    }

    #[test]
    fn acknowledgement_has_an_explicit_kind() {
        let frame = StreamFrame::acknowledgement(9, b"accepted".to_vec());
        let decoded = StreamFrame::decode(&frame.encode()).unwrap();

        assert_eq!(decoded.kind, StreamFrameKind::Acknowledgement);
        assert_eq!(decoded.sequence, 9);
    }

    #[test]
    fn malformed_unknown_version_and_unknown_kind_are_distinct() {
        assert_eq!(
            StreamFrame::decode(&[0xff]),
            Err(StreamProtocolError::MalformedEnvelope)
        );

        let unknown_version = wire::StreamEnvelope {
            protocol_version: PROTOCOL_VERSION + 1,
            sequence: 1,
            kind: wire::FrameKind::Data.into(),
            payload: Vec::new(),
        }
        .encode_to_vec();
        assert_eq!(
            StreamFrame::decode(&unknown_version),
            Err(StreamProtocolError::UnsupportedProtocolVersion(2))
        );

        let unknown_kind = wire::StreamEnvelope {
            protocol_version: PROTOCOL_VERSION,
            sequence: 1,
            kind: 99,
            payload: Vec::new(),
        }
        .encode_to_vec();
        assert_eq!(
            StreamFrame::decode(&unknown_kind),
            Err(StreamProtocolError::UnknownFrameKind(99))
        );
    }
}
