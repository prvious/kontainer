//! Follow one service's logs into one connection: validate, open the manager's stream, then
//! forward lines through the backlog until the service goes, the manager stops answering, the
//! access credential expires, or the connection ends.

use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use kontainer_engine::{LogSource, ServiceLogLine};
use kontainer_stream_protocol::{
    LogEnd, LogEndReason, LogLine, LogMessage, LogStreamKind, LogsOpen,
};
use kontainer_stream_transport::{OutboundFrameError, OutboundSender, StreamClose};
use tokio::time;
use tokio_util::sync::CancellationToken;

use crate::{
    logs::{
        backlog::{Backlog, Flush},
        domain::{BACKLOG_LINES, truncate_line, validate_open},
    },
    services::actions::list_services::MANAGER_READ_DEADLINE,
    swarm::engine::{ManagerEngine, ManagerError},
};

/// Closed when the access credential the connection opened with expires.
pub const ACCESS_EXPIRED: StreamClose = StreamClose::new(4401, "access expired");
const FLUSH_EVERY: Duration = Duration::from_millis(25);
/// How long the last frames may wait for room before the connection closes anyway.
const END_GRACE: Duration = Duration::from_secs(1);

pub struct LogStream {
    pub manager: Arc<dyn ManagerEngine>,
    pub outbound: OutboundSender,
    pub access_expires_at: DateTime<Utc>,
    pub liveness: Duration,
    pub cancellation: CancellationToken,
}

impl LogStream {
    pub async fn run(self, open: LogsOpen) {
        let mut frames = Frames::new(self.outbound.clone());
        let request = match validate_open(&open) {
            Ok(request) => request,
            Err(_) => {
                return frames
                    .end(
                        LogEndReason::Error,
                        "invalid_request",
                        "The log request was refused.",
                    )
                    .await;
            }
        };
        // The route checked the manager answers before upgrading.
        let mut lines = match self
            .manager
            .service_logs(&open.service_id, request, &self.cancellation)
            .await
        {
            Ok(lines) => lines,
            Err(ManagerError::NotFound) => {
                return frames
                    .end(LogEndReason::Error, "not_found", "No service has this id.")
                    .await;
            }
            Err(ManagerError::InvalidInput) => {
                return frames
                    .end(
                        LogEndReason::Error,
                        "invalid_request",
                        "The log request was refused.",
                    )
                    .await;
            }
            Err(ManagerError::Cancelled) => return,
            Err(_) => return frames.end_unavailable().await,
        };

        let expiry = time::sleep(
            (self.access_expires_at - Utc::now())
                .to_std()
                .unwrap_or_default(),
        );
        tokio::pin!(expiry);
        let mut liveness = time::interval_at(time::Instant::now() + self.liveness, self.liveness);
        let mut flush = time::interval(FLUSH_EVERY);
        let mut seq = 0_u64;
        loop {
            tokio::select! {
                biased;
                () = self.cancellation.cancelled() => return,
                () = self.outbound.closed() => return,
                () = &mut expiry => {
                    self.outbound.close(ACCESS_EXPIRED);
                    return;
                }
                // Docker keeps following a removed service, so each check looks for it too.
                _ = liveness.tick() => match self.service_present(&open.service_id).await {
                    Some(true) => {}
                    Some(false) => return frames.end_removed().await,
                    None => return frames.end_unavailable().await,
                },
                item = lines.next() => match item {
                    Some(Ok(line)) => {
                        seq += 1;
                        frames.backlog.push(log_line(seq, line));
                        if frames.flush().is_err() {
                            return;
                        }
                    }
                    Some(Err(_)) => return frames.end_unavailable().await,
                    None => {
                        if self.service_present(&open.service_id).await == Some(false) {
                            return frames.end_removed().await;
                        }
                        return frames
                            .end(LogEndReason::Error, "stream_ended", "The manager ended the log stream.")
                            .await;
                    }
                },
                _ = flush.tick(), if !frames.backlog.is_empty() => {
                    if frames.flush().is_err() {
                        return;
                    }
                }
            }
        }
    }

    /// Whether the manager still lists the service; `None` when it didn't answer in time.
    async fn service_present(&self, service_id: &str) -> Option<bool> {
        time::timeout(
            MANAGER_READ_DEADLINE,
            self.manager.list_services(&self.cancellation),
        )
        .await
        .ok()?
        .ok()
        .map(|services| services.iter().any(|service| service.id == service_id))
    }
}

/// Numbers envelopes for one connection.
struct FrameSender {
    outbound: OutboundSender,
    sequence: u64,
}

impl FrameSender {
    fn send(&mut self, message: LogMessage) -> Result<(), OutboundFrameError> {
        self.outbound
            .try_send(message.into_frame(self.sequence + 1))?;
        self.sequence += 1;
        Ok(())
    }
}

/// The connection's sender and the lines waiting for room in it.
struct Frames {
    sender: FrameSender,
    backlog: Backlog,
}

impl Frames {
    fn new(outbound: OutboundSender) -> Self {
        Self {
            sender: FrameSender {
                outbound,
                sequence: 0,
            },
            backlog: Backlog::new(BACKLOG_LINES),
        }
    }

    fn flush(&mut self) -> Result<Flush, OutboundFrameError> {
        self.backlog.flush(|message| self.sender.send(message))
    }

    async fn end_removed(&mut self) {
        self.end(
            LogEndReason::ServiceRemoved,
            "service_removed",
            "The service was removed.",
        )
        .await;
    }

    async fn end_unavailable(&mut self) {
        self.end(
            LogEndReason::Error,
            "manager_unavailable",
            "The Swarm manager didn't answer.",
        )
        .await;
    }

    /// Write what is waiting and the end, giving the client a moment to make room, then close.
    async fn end(&mut self, reason: LogEndReason, code: &str, message: &str) {
        let end = LogMessage::End(LogEnd {
            reason,
            code: code.to_owned(),
            message: message.to_owned(),
        });
        let _ = time::timeout(END_GRACE, async {
            loop {
                match self.flush() {
                    Ok(Flush::Drained) => match self.sender.send(end.clone()) {
                        Ok(()) => return,
                        Err(OutboundFrameError::QueueOverloaded) => {}
                        Err(_) => return,
                    },
                    Ok(Flush::Blocked) => {}
                    Err(_) => return,
                }
                time::sleep(FLUSH_EVERY).await;
            }
        })
        .await;
        self.sender.outbound.close(StreamClose::NORMAL);
    }
}

fn log_line(seq: u64, line: ServiceLogLine) -> LogLine {
    let (text, truncated) = truncate_line(line.text);
    LogLine {
        seq,
        ts_unix_ms: line.timestamp_unix_ms,
        stream: match line.source {
            LogSource::Stdout => LogStreamKind::Stdout,
            LogSource::Stderr => LogStreamKind::Stderr,
        },
        task_id: line.task_id,
        node_id: line.node_id,
        line: text,
        truncated,
    }
}
