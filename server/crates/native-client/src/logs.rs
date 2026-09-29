//! `GET /api/v1/ws/logs`: one service's logs over the binary WebSocket transport (ADR 0006,
//! ADR 0025).
//!
//! [`NativeClient::connect_logs`] makes one authenticated upgrade and sends the open. The caller
//! owns credential custody and wraps that call in a [`LogConnector`]; [`LogFollower`] drives it:
//! reconnecting with backoff, reopening from just before the last line it delivered, dropping
//! the lines it already delivered, and reporting the transitions as [`LogEvent`]s. Nothing here
//! spawns work: the caller's task pulls events.

use std::{collections::VecDeque, future::Future, pin::Pin, sync::Arc, time::Duration};

use kontainer_stream_protocol::{LogEndReason, LogMessage, LogStreamKind, LogsOpen, MAX_LOG_TAIL};
use kontainer_stream_transport::{NativeStreamClient, StreamClientError, StreamLimits};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::client::NativeClient;
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;
use crate::services::ServiceId;

/// First reconnect delay; it doubles up to [`MAX_RECONNECT_DELAY`].
pub const FIRST_RECONNECT_DELAY: Duration = Duration::from_secs(1);
pub const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(30);
/// How far before the last delivered line a reconnect reopens.
pub const RECONNECT_OVERLAP_MS: i64 = 1_000;
/// Close code for an access credential that expired mid-stream.
pub const ACCESS_EXPIRED_CLOSE: u16 = 4401;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    Stdout,
    Stderr,
}

/// One delivered line. `seq` counts lines delivered by this follower, across reconnects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLineItem {
    pub seq: u64,
    pub ts_unix_ms: i64,
    pub stream: LogStream,
    pub task_id: String,
    pub node_id: String,
    pub text: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogEndKind {
    ServiceRemoved,
    Error,
}

/// What one connection yields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogFrame {
    Line(LogLineItem),
    Gap {
        dropped: u64,
    },
    End {
        reason: LogEndKind,
        code: String,
        message: String,
    },
}

/// Why a connection stopped yielding frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogDisconnect {
    /// Closed 4401: reconnect, and the connector refreshes the credential.
    AccessExpired,
    Dropped,
    Cancelled,
}

/// What a [`LogReceiver`] yields next.
pub type LogFrameFuture<'a> =
    Pin<Box<dyn Future<Output = Result<LogFrame, LogDisconnect>> + Send + 'a>>;
/// What one [`LogConnector::connect`] resolves to.
pub type LogConnectFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Box<dyn LogReceiver>, LogConnectError>> + Send + 'a>>;

/// One open log connection.
pub trait LogReceiver: Send {
    fn next_frame<'a>(&'a mut self, cancellation: &'a CancellationToken) -> LogFrameFuture<'a>;
}

/// What to open: the first connection uses the caller's window; reconnects reopen from just
/// before the last delivered line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogOpenRequest {
    pub service_id: ServiceId,
    pub since_unix_ms: Option<i64>,
    pub tail: u32,
}

/// Why a connect failed, as far as retrying goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogConnectError {
    /// Worth another try after the backoff.
    Transient,
    /// Stop and say so, e.g. `relogin_required` or `limit_reached`.
    Terminal(String),
}

/// Opens connections for a [`LogFollower`], owning credentials and their refresh.
pub trait LogConnector: Send {
    fn connect<'a>(
        &'a mut self,
        request: LogOpenRequest,
        cancellation: &'a CancellationToken,
    ) -> LogConnectFuture<'a>;
}

/// What a follower reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogEvent {
    Line(LogLineItem),
    Gap {
        dropped: u64,
    },
    Reconnecting {
        attempt: u32,
        next_retry_ms: u64,
    },
    Reconnected,
    Ended {
        reason: LogEndKind,
        code: String,
        message: String,
    },
}

/// Server end codes that mean "try again" rather than "this stream is over".
fn transient_end(code: &str) -> bool {
    matches!(code, "manager_unavailable" | "stream_ended")
}

/// 1 s, 2 s, 4 s, … capped at 30 s.
#[must_use]
pub fn reconnect_delay(attempt: u32) -> Duration {
    let doublings = attempt.saturating_sub(1).min(16);
    FIRST_RECONNECT_DELAY
        .saturating_mul(1 << doublings)
        .min(MAX_RECONNECT_DELAY)
}

/// Lines delivered in the last second before the newest one, to drop repeats after a reopen.
#[derive(Debug, Default)]
pub struct Overlap {
    last_ts: Option<i64>,
    recent: VecDeque<(i64, String, String)>,
}

impl Overlap {
    /// Whether `line` is new; records it when it is.
    pub fn accept(&mut self, line: &LogLineItem) -> bool {
        if let Some(last) = self.last_ts {
            if line.ts_unix_ms < last - RECONNECT_OVERLAP_MS {
                return false;
            }
            if self.recent.iter().any(|(ts, task, text)| {
                *ts == line.ts_unix_ms && *task == line.task_id && *text == line.text
            }) {
                return false;
            }
        }
        let last = self
            .last_ts
            .map_or(line.ts_unix_ms, |last| last.max(line.ts_unix_ms));
        self.last_ts = Some(last);
        self.recent
            .push_back((line.ts_unix_ms, line.task_id.clone(), line.text.clone()));
        while self
            .recent
            .front()
            .is_some_and(|(ts, _, _)| *ts < last - RECONNECT_OVERLAP_MS)
        {
            self.recent.pop_front();
        }
        true
    }

    #[must_use]
    pub fn last_ts(&self) -> Option<i64> {
        self.last_ts
    }
}

enum FollowState {
    Connect,
    Wait(Duration),
    Connected(Box<dyn LogReceiver>),
    Done,
}

/// Follows one service across reconnects; see the module docs.
pub struct LogFollower<C> {
    connector: C,
    first: LogOpenRequest,
    state: FollowState,
    overlap: Overlap,
    delivered: u64,
    attempt: u32,
    retry_now: Arc<Notify>,
    queued: VecDeque<LogEvent>,
}

impl<C: LogConnector> LogFollower<C> {
    pub fn new(connector: C, request: LogOpenRequest) -> Self {
        Self {
            connector,
            first: request,
            state: FollowState::Connect,
            overlap: Overlap::default(),
            delivered: 0,
            attempt: 0,
            retry_now: Arc::new(Notify::new()),
            queued: VecDeque::new(),
        }
    }

    /// Wakes a follower waiting out its backoff so it reconnects now.
    #[must_use]
    pub fn retry_handle(&self) -> Arc<Notify> {
        self.retry_now.clone()
    }

    fn reopen(&self) -> LogOpenRequest {
        match self.overlap.last_ts() {
            Some(last) => LogOpenRequest {
                service_id: self.first.service_id.clone(),
                since_unix_ms: Some(last - RECONNECT_OVERLAP_MS),
                tail: MAX_LOG_TAIL,
            },
            None => self.first.clone(),
        }
    }

    fn back_off(&mut self) {
        self.attempt += 1;
        let delay = reconnect_delay(self.attempt);
        self.queued.push_back(LogEvent::Reconnecting {
            attempt: self.attempt,
            next_retry_ms: u64::try_from(delay.as_millis()).unwrap_or(u64::MAX),
        });
        self.state = FollowState::Wait(delay);
    }

    fn finish(&mut self, reason: LogEndKind, code: String, message: String) {
        self.queued.push_back(LogEvent::Ended {
            reason,
            code,
            message,
        });
        self.state = FollowState::Done;
    }

    /// The next event, or `None` once the follow ended or `cancellation` fired.
    pub async fn next(&mut self, cancellation: &CancellationToken) -> Option<LogEvent> {
        loop {
            if let Some(event) = self.queued.pop_front() {
                return Some(event);
            }
            match &mut self.state {
                FollowState::Done => return None,
                FollowState::Wait(delay) => {
                    let delay = *delay;
                    tokio::select! {
                        biased;
                        () = cancellation.cancelled() => return None,
                        () = self.retry_now.notified() => {}
                        () = tokio::time::sleep(delay) => {}
                    }
                    self.state = FollowState::Connect;
                }
                FollowState::Connect => {
                    let request = self.reopen();
                    match self.connector.connect(request, cancellation).await {
                        Ok(receiver) => {
                            if self.attempt > 0 {
                                self.queued.push_back(LogEvent::Reconnected);
                            }
                            self.attempt = 0;
                            self.state = FollowState::Connected(receiver);
                        }
                        Err(_) if cancellation.is_cancelled() => return None,
                        Err(LogConnectError::Transient) => self.back_off(),
                        Err(LogConnectError::Terminal(code)) => self.finish(
                            LogEndKind::Error,
                            code,
                            "The log stream could not be opened.".to_owned(),
                        ),
                    }
                }
                FollowState::Connected(receiver) => match receiver.next_frame(cancellation).await {
                    Ok(LogFrame::Line(mut line)) => {
                        if self.overlap.accept(&line) {
                            self.delivered += 1;
                            line.seq = self.delivered;
                            return Some(LogEvent::Line(line));
                        }
                    }
                    Ok(LogFrame::Gap { dropped }) => return Some(LogEvent::Gap { dropped }),
                    Ok(LogFrame::End {
                        reason: LogEndKind::Error,
                        code,
                        ..
                    }) if transient_end(&code) => self.back_off(),
                    Ok(LogFrame::End {
                        reason,
                        code,
                        message,
                    }) => self.finish(reason, code, message),
                    Err(LogDisconnect::AccessExpired) => self.state = FollowState::Connect,
                    Err(LogDisconnect::Dropped) => self.back_off(),
                    Err(LogDisconnect::Cancelled) => return None,
                },
            }
        }
    }
}

/// A connection over the transport's native client.
struct WebSocketLogReceiver(NativeStreamClient);

impl LogReceiver for WebSocketLogReceiver {
    fn next_frame<'a>(&'a mut self, cancellation: &'a CancellationToken) -> LogFrameFuture<'a> {
        Box::pin(async move {
            let frame = match self.0.receive_frame(cancellation).await {
                Ok(frame) => frame,
                Err(StreamClientError::Cancelled) => return Err(LogDisconnect::Cancelled),
                Err(StreamClientError::Closed { code, .. }) if code == ACCESS_EXPIRED_CLOSE => {
                    return Err(LogDisconnect::AccessExpired);
                }
                Err(_) => return Err(LogDisconnect::Dropped),
            };
            match LogMessage::from_frame(&frame) {
                Ok(LogMessage::Line(line)) => Ok(LogFrame::Line(LogLineItem {
                    seq: line.seq,
                    ts_unix_ms: line.ts_unix_ms,
                    stream: match line.stream {
                        LogStreamKind::Stdout => LogStream::Stdout,
                        LogStreamKind::Stderr => LogStream::Stderr,
                    },
                    task_id: line.task_id,
                    node_id: line.node_id,
                    text: line.line,
                    truncated: line.truncated,
                })),
                Ok(LogMessage::Gap(gap)) => Ok(LogFrame::Gap {
                    dropped: gap.dropped,
                }),
                Ok(LogMessage::End(end)) => Ok(LogFrame::End {
                    reason: match end.reason {
                        LogEndReason::ServiceRemoved => LogEndKind::ServiceRemoved,
                        LogEndReason::Error => LogEndKind::Error,
                    },
                    code: end.code,
                    message: end.message,
                }),
                // A server that sends anything else is not one to keep reading.
                _ => Err(LogDisconnect::Dropped),
            }
        })
    }
}

impl NativeClient {
    /// Upgrade to the log stream with `access` and send the open. A 401 is `Unauthorized`, so
    /// the caller can refresh and try again; a 429 is `LimitReached`.
    pub async fn connect_logs(
        &self,
        access: &AccessCredential,
        request: &LogOpenRequest,
        cancellation: &CancellationToken,
    ) -> Result<Box<dyn LogReceiver>, NativeClientError> {
        // WebSockets verify with the public roots only; a custom bundle is refused, not ignored.
        if self.custom_trust {
            return Err(NativeClientError::InvalidConfiguration);
        }
        let mut endpoint = self.endpoint("/api/v1/ws/logs")?;
        let scheme = if endpoint.scheme() == "https" {
            "wss"
        } else {
            "ws"
        };
        endpoint
            .set_scheme(scheme)
            .map_err(|()| NativeClientError::InvalidConfiguration)?;
        let limits = StreamLimits::default();
        let mut client = NativeStreamClient::connect_with_bearer(
            endpoint.as_str(),
            Some(access.expose()),
            limits,
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
        let open = LogMessage::Open(LogsOpen {
            service_id: request.service_id.as_str().to_owned(),
            since_unix_ms: request.since_unix_ms,
            tail: Some(request.tail.min(MAX_LOG_TAIL)),
        });
        client
            .send_frame(&open.into_frame(1), cancellation)
            .await
            .map_err(|error| match error {
                StreamClientError::Cancelled => NativeClientError::Cancelled,
                _ => NativeClientError::Unavailable,
            })?;
        Ok(Box::new(WebSocketLogReceiver(client)))
    }
}
