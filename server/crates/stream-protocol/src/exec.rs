//! Exec payloads: one message per frame kind, carried in the envelope's `payload`.

use prost::Message;

use crate::{StreamFrame, StreamFrameKind, wire};

/// Terminal columns and rows are at most this; larger values are refused.
pub const MAX_EXEC_DIMENSION: u32 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecOpen {
    pub node_id: String,
    pub container_id: String,
    pub shell: Option<String>,
    pub cols: u32,
    pub rows: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecResize {
    pub cols: u32,
    pub rows: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecStarted {
    pub exec_id: String,
    pub shell: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecExit {
    /// `None` when the exit status is unknown; never a made-up 0.
    pub code: Option<i64>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecError {
    pub code: String,
    pub message: String,
}

/// Every exec payload, in either direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecMessage {
    Open(ExecOpen),
    Input(Vec<u8>),
    Resize(ExecResize),
    Close,
    Started(ExecStarted),
    Output(Vec<u8>),
    Exit(ExecExit),
    Error(ExecError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecPayloadError {
    /// The frame kind carries no exec payload.
    NotAnExecFrame,
    Malformed,
}

impl ExecMessage {
    #[must_use]
    pub fn into_frame(self, sequence: u64) -> StreamFrame {
        let (kind, payload) = match self {
            Self::Open(open) => (
                StreamFrameKind::ExecOpen,
                wire::ExecOpen {
                    node_id: open.node_id,
                    container_id: open.container_id,
                    shell: open.shell,
                    cols: open.cols,
                    rows: open.rows,
                }
                .encode_to_vec(),
            ),
            Self::Input(bytes) => (
                StreamFrameKind::ExecInput,
                wire::ExecInput { bytes }.encode_to_vec(),
            ),
            Self::Resize(resize) => (
                StreamFrameKind::ExecResize,
                wire::ExecResize {
                    cols: resize.cols,
                    rows: resize.rows,
                }
                .encode_to_vec(),
            ),
            Self::Close => (
                StreamFrameKind::ExecClose,
                wire::ExecClose {}.encode_to_vec(),
            ),
            Self::Started(started) => (
                StreamFrameKind::ExecStarted,
                wire::ExecStarted {
                    exec_id: started.exec_id,
                    shell: started.shell,
                }
                .encode_to_vec(),
            ),
            Self::Output(bytes) => (
                StreamFrameKind::ExecOutput,
                wire::ExecOutput { bytes }.encode_to_vec(),
            ),
            Self::Exit(exit) => (
                StreamFrameKind::ExecExit,
                wire::ExecExit {
                    code: exit.code,
                    reason: exit.reason,
                }
                .encode_to_vec(),
            ),
            Self::Error(error) => (
                StreamFrameKind::ExecError,
                wire::ExecError {
                    code: error.code,
                    message: error.message,
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

    pub fn from_frame(frame: &StreamFrame) -> Result<Self, ExecPayloadError> {
        let bytes = frame.payload.as_slice();
        let malformed = |_| ExecPayloadError::Malformed;
        Ok(match frame.kind {
            StreamFrameKind::ExecOpen => {
                let open = wire::ExecOpen::decode(bytes).map_err(malformed)?;
                Self::Open(ExecOpen {
                    node_id: open.node_id,
                    container_id: open.container_id,
                    shell: open.shell,
                    cols: open.cols,
                    rows: open.rows,
                })
            }
            StreamFrameKind::ExecInput => {
                Self::Input(wire::ExecInput::decode(bytes).map_err(malformed)?.bytes)
            }
            StreamFrameKind::ExecResize => {
                let resize = wire::ExecResize::decode(bytes).map_err(malformed)?;
                Self::Resize(ExecResize {
                    cols: resize.cols,
                    rows: resize.rows,
                })
            }
            StreamFrameKind::ExecClose => {
                wire::ExecClose::decode(bytes).map_err(malformed)?;
                Self::Close
            }
            StreamFrameKind::ExecStarted => {
                let started = wire::ExecStarted::decode(bytes).map_err(malformed)?;
                Self::Started(ExecStarted {
                    exec_id: started.exec_id,
                    shell: started.shell,
                })
            }
            StreamFrameKind::ExecOutput => {
                Self::Output(wire::ExecOutput::decode(bytes).map_err(malformed)?.bytes)
            }
            StreamFrameKind::ExecExit => {
                let exit = wire::ExecExit::decode(bytes).map_err(malformed)?;
                Self::Exit(ExecExit {
                    code: exit.code,
                    reason: exit.reason,
                })
            }
            StreamFrameKind::ExecError => {
                let error = wire::ExecError::decode(bytes).map_err(malformed)?;
                Self::Error(ExecError {
                    code: error.code,
                    message: error.message,
                })
            }
            _ => return Err(ExecPayloadError::NotAnExecFrame),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ExecError, ExecExit, ExecMessage, ExecOpen, ExecPayloadError, ExecResize, ExecStarted,
    };
    use crate::{LogMessage, StreamFrame, StreamFrameKind};

    #[test]
    fn every_exec_message_round_trips_through_an_envelope() {
        let messages = [
            ExecMessage::Open(ExecOpen {
                node_id: "manager".to_owned(),
                container_id: "a".repeat(64),
                shell: Some("/bin/zsh".to_owned()),
                cols: 120,
                rows: 40,
            }),
            ExecMessage::Open(ExecOpen {
                node_id: "manager".to_owned(),
                container_id: "a".repeat(64),
                shell: None,
                cols: 80,
                rows: 24,
            }),
            ExecMessage::Input(b"echo ok\r".to_vec()),
            ExecMessage::Resize(ExecResize {
                cols: 100,
                rows: 30,
            }),
            ExecMessage::Close,
            ExecMessage::Started(ExecStarted {
                exec_id: "e".repeat(64),
                shell: "/bin/sh".to_owned(),
            }),
            ExecMessage::Output(vec![0x1b, b'[', b'0', b'm', 0xff]),
            ExecMessage::Exit(ExecExit {
                code: Some(0),
                reason: "exited".to_owned(),
            }),
            ExecMessage::Exit(ExecExit {
                code: None,
                reason: "idle_timeout".to_owned(),
            }),
            ExecMessage::Error(ExecError {
                code: "shell_not_found".to_owned(),
                message: "No shell at /bin/zsh.".to_owned(),
            }),
        ];
        for (sequence, message) in messages.into_iter().enumerate() {
            let frame = message.clone().into_frame(sequence as u64);
            let decoded = StreamFrame::decode(&frame.encode()).unwrap();
            assert_eq!(ExecMessage::from_frame(&decoded).unwrap(), message);
        }
    }

    #[test]
    fn exec_and_log_payloads_refuse_each_others_frames() {
        let output = ExecMessage::Output(b"x".to_vec()).into_frame(1);
        assert!(LogMessage::from_frame(&output).is_err());
        assert_eq!(
            ExecMessage::from_frame(&LogMessage::Close.into_frame(1)),
            Err(ExecPayloadError::NotAnExecFrame)
        );
        let garbage = StreamFrame {
            sequence: 1,
            kind: StreamFrameKind::ExecOpen,
            payload: vec![0xff, 0xff],
        };
        assert_eq!(
            ExecMessage::from_frame(&garbage),
            Err(ExecPayloadError::Malformed)
        );
    }
}
