//! `GET /api/v1/ws/exec`: an interactive shell in one container over the binary WebSocket
//! transport (ADR 0006, ADR 0026).
//!
//! [`NativeClient::connect_exec`] makes one authenticated upgrade, sends the open, and splits the
//! connection: an [`ExecInputSink`] for keystrokes, resizes, and the close, and an
//! [`ExecEventSource`] the caller's task reads. There is no reconnect: a new shell is a new open.

use std::{future::Future, pin::Pin};

use kontainer_stream_protocol::{ExecMessage, ExecOpen, ExecResize, MAX_EXEC_DIMENSION};
use kontainer_stream_transport::{
    NativeStreamClient, NativeStreamReceiver, NativeStreamSender, StreamClientError, StreamLimits,
};
use tokio_util::sync::CancellationToken;

use crate::client::NativeClient;
use crate::containers::NodeId;
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;

/// Input frames carry at most this much; longer pastes are split.
pub const MAX_EXEC_INPUT: usize = 32 * 1024;
const MAX_SHELL_LEN: usize = 256;

/// What to open. The container id is Docker's full 64-hex id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecOpenRequest {
    pub node_id: NodeId,
    pub container_id: String,
    pub shell: Option<String>,
    pub cols: u32,
    pub rows: u32,
}

impl ExecOpenRequest {
    /// Refuse locally what the controller would refuse.
    pub fn validate(&self) -> Result<(), NativeClientError> {
        let hex = self.container_id.len() == 64
            && self
                .container_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        let shell = self.shell.as_deref().is_none_or(|shell| {
            !shell.is_empty()
                && shell.len() <= MAX_SHELL_LEN
                && !shell.chars().any(char::is_control)
        });
        if hex && shell && valid_dimensions(self.cols, self.rows) {
            Ok(())
        } else {
            Err(NativeClientError::InvalidRequest)
        }
    }
}

#[must_use]
pub fn valid_dimensions(cols: u32, rows: u32) -> bool {
    (1..=MAX_EXEC_DIMENSION).contains(&cols) && (1..=MAX_EXEC_DIMENSION).contains(&rows)
}

/// What the caller sends after the open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecInputFrame {
    Input(Vec<u8>),
    Resize { cols: u32, rows: u32 },
    Close,
}

/// Why the connection closed, as the desktop names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecClosedReason {
    /// The controller closed normally, after an exit or an error.
    Closed,
    SessionExpired,
    SlowConsumer,
    ServerShutdown,
    ConnectionLost,
}

impl ExecClosedReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::SessionExpired => "session_expired",
            Self::SlowConsumer => "slow_consumer",
            Self::ServerShutdown => "server_shutdown",
            Self::ConnectionLost => "connection_lost",
        }
    }

    fn from_close(code: u16) -> Self {
        match code {
            1000 => Self::Closed,
            4401 => Self::SessionExpired,
            1013 => Self::SlowConsumer,
            1001 => Self::ServerShutdown,
            _ => Self::ConnectionLost,
        }
    }
}

/// What a shell session reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecEvent {
    Started {
        exec_id: String,
        shell: String,
    },
    Output(Vec<u8>),
    /// `code` is `None` when the exit status is unknown.
    Exit {
        code: Option<i64>,
        reason: String,
    },
    Error {
        code: String,
        message: String,
    },
    /// The last event: the connection is gone.
    Closed {
        reason: ExecClosedReason,
    },
}

pub type ExecSendFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), NativeClientError>> + Send + 'a>>;
pub type ExecEventFuture<'a> = Pin<Box<dyn Future<Output = Option<ExecEvent>> + Send + 'a>>;

/// The writing half of a session.
pub trait ExecInputSink: Send {
    fn send<'a>(
        &'a mut self,
        frame: ExecInputFrame,
        cancellation: &'a CancellationToken,
    ) -> ExecSendFuture<'a>;
}

/// The reading half; `None` after [`ExecEvent::Closed`].
pub trait ExecEventSource: Send {
    fn next_event<'a>(&'a mut self, cancellation: &'a CancellationToken) -> ExecEventFuture<'a>;
}

pub struct ExecConnection {
    pub input: Box<dyn ExecInputSink>,
    pub events: Box<dyn ExecEventSource>,
}

struct WebSocketExecInput {
    sender: NativeStreamSender,
    sequence: u64,
}

impl ExecInputSink for WebSocketExecInput {
    fn send<'a>(
        &'a mut self,
        frame: ExecInputFrame,
        cancellation: &'a CancellationToken,
    ) -> ExecSendFuture<'a> {
        Box::pin(async move {
            let messages = match frame {
                ExecInputFrame::Input(bytes) => bytes
                    .chunks(MAX_EXEC_INPUT)
                    .map(|chunk| ExecMessage::Input(chunk.to_vec()))
                    .collect(),
                ExecInputFrame::Resize { cols, rows } if valid_dimensions(cols, rows) => {
                    vec![ExecMessage::Resize(ExecResize { cols, rows })]
                }
                ExecInputFrame::Resize { .. } => return Err(NativeClientError::InvalidRequest),
                ExecInputFrame::Close => vec![ExecMessage::Close],
            };
            for message in messages {
                self.sequence += 1;
                self.sender
                    .send_frame(&message.into_frame(self.sequence), cancellation)
                    .await
                    .map_err(|error| match error {
                        StreamClientError::Cancelled => NativeClientError::Cancelled,
                        _ => NativeClientError::Unavailable,
                    })?;
            }
            Ok(())
        })
    }
}

struct WebSocketExecEvents {
    receiver: NativeStreamReceiver,
    done: bool,
}

impl ExecEventSource for WebSocketExecEvents {
    fn next_event<'a>(&'a mut self, cancellation: &'a CancellationToken) -> ExecEventFuture<'a> {
        Box::pin(async move {
            if self.done {
                return None;
            }
            let frame = match self.receiver.receive_frame(cancellation).await {
                Ok(frame) => frame,
                Err(StreamClientError::Cancelled) => return None,
                Err(error) => {
                    self.done = true;
                    let reason = match error {
                        StreamClientError::Closed { code, .. } => {
                            ExecClosedReason::from_close(code)
                        }
                        _ => ExecClosedReason::ConnectionLost,
                    };
                    return Some(ExecEvent::Closed { reason });
                }
            };
            Some(match ExecMessage::from_frame(&frame) {
                Ok(ExecMessage::Started(started)) => ExecEvent::Started {
                    exec_id: started.exec_id,
                    shell: started.shell,
                },
                Ok(ExecMessage::Output(bytes)) => ExecEvent::Output(bytes),
                Ok(ExecMessage::Exit(exit)) => ExecEvent::Exit {
                    code: exit.code,
                    reason: exit.reason,
                },
                Ok(ExecMessage::Error(error)) => ExecEvent::Error {
                    code: error.code,
                    message: error.message,
                },
                // Frames only a client sends: a server that sends them is not one to trust.
                _ => {
                    self.done = true;
                    ExecEvent::Closed {
                        reason: ExecClosedReason::ConnectionLost,
                    }
                }
            })
        })
    }
}

impl NativeClient {
    /// Upgrade to an exec session with `access` and send the open. A 401 is `Unauthorized`, so
    /// the caller can refresh and try again; a 429 is `LimitReached`.
    pub async fn connect_exec(
        &self,
        access: &AccessCredential,
        request: &ExecOpenRequest,
        cancellation: &CancellationToken,
    ) -> Result<ExecConnection, NativeClientError> {
        request.validate()?;
        // WebSockets verify with the public roots only; a custom bundle is refused, not ignored.
        if self.custom_trust {
            return Err(NativeClientError::InvalidConfiguration);
        }
        let mut endpoint = self.endpoint("/api/v1/ws/exec")?;
        let scheme = if endpoint.scheme() == "https" {
            "wss"
        } else {
            "ws"
        };
        endpoint
            .set_scheme(scheme)
            .map_err(|()| NativeClientError::InvalidConfiguration)?;
        let mut client = NativeStreamClient::connect_with_bearer(
            endpoint.as_str(),
            Some(access.expose()),
            StreamLimits::default(),
            cancellation,
        )
        .await
        .map_err(|error| match error {
            StreamClientError::HandshakeRejected { status: 401 } => NativeClientError::Unauthorized,
            StreamClientError::HandshakeRejected { status: 429 } => NativeClientError::LimitReached,
            StreamClientError::HandshakeRejected { status: 400 } => {
                NativeClientError::InvalidRequest
            }
            StreamClientError::HandshakeRejected { status: 503 } => {
                NativeClientError::ServiceUnavailable
            }
            StreamClientError::Cancelled => NativeClientError::Cancelled,
            StreamClientError::InvalidEndpoint => NativeClientError::InvalidConfiguration,
            _ => NativeClientError::Unavailable,
        })?;
        let open = ExecMessage::Open(ExecOpen {
            node_id: request.node_id.as_str().to_owned(),
            container_id: request.container_id.clone(),
            shell: request.shell.clone(),
            cols: request.cols,
            rows: request.rows,
        });
        client
            .send_frame(&open.into_frame(1), cancellation)
            .await
            .map_err(|error| match error {
                StreamClientError::Cancelled => NativeClientError::Cancelled,
                _ => NativeClientError::Unavailable,
            })?;
        let (sender, receiver) = client.split();
        Ok(ExecConnection {
            input: Box::new(WebSocketExecInput {
                sender,
                sequence: 1,
            }),
            events: Box::new(WebSocketExecEvents {
                receiver,
                done: false,
            }),
        })
    }
}
