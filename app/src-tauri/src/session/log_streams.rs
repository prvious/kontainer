//! Open log streams (ADR 0006, ADR 0025): each follows one service through the native client's
//! [`LogFollower`] in its own task and pushes events to React over a Channel, batching lines
//! every 25 ms. The registry maps stream ids to cancellation; logout and window close cancel all.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use kontainer_native_client::{
    LogConnectFuture, LogConnector, LogEndKind, LogEvent, LogFollower, LogLineItem, LogOpenRequest,
    LogStream,
};
use serde::Serialize;
use tauri::{ipc::Channel, AppHandle, Manager};
use tokio::sync::{mpsc, Notify};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::manager::{log_connect_error, SessionManager};

/// Lines are collected this long before they cross to React.
pub(crate) const LINE_BATCH_WINDOW: Duration = Duration::from_millis(25);
/// A batch goes out early once it holds this many lines.
const MAX_BATCH_LINES: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LogLineView {
    seq: u64,
    /// Epoch milliseconds.
    ts: i64,
    stream: &'static str,
    task_id: String,
    node_id: String,
    text: String,
    truncated: bool,
}

impl From<LogLineItem> for LogLineView {
    fn from(line: LogLineItem) -> Self {
        Self {
            seq: line.seq,
            ts: line.ts_unix_ms,
            stream: match line.stream {
                LogStream::Stdout => "stdout",
                LogStream::Stderr => "stderr",
            },
            task_id: line.task_id,
            node_id: line.node_id,
            text: line.text,
            truncated: line.truncated,
        }
    }
}

/// What a stream's Channel carries; mirrors `LogStreamEvent` in `app/src/types/logs.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum LogStreamEvent {
    Lines {
        lines: Vec<LogLineView>,
    },
    Gap {
        dropped: u64,
    },
    Reconnecting {
        attempt: u32,
        next_retry_ms: u64,
    },
    Reconnected,
    Ended {
        reason: &'static str,
        code: String,
        message: String,
    },
}

impl LogStreamEvent {
    fn from_event(event: LogEvent) -> Option<Self> {
        Some(match event {
            LogEvent::Line(_) => return None,
            LogEvent::Gap { dropped } => Self::Gap { dropped },
            LogEvent::Reconnecting {
                attempt,
                next_retry_ms,
            } => Self::Reconnecting {
                attempt,
                next_retry_ms,
            },
            LogEvent::Reconnected => Self::Reconnected,
            LogEvent::Ended {
                reason,
                code,
                message,
            } => Self::Ended {
                reason: match reason {
                    LogEndKind::ServiceRemoved => "service_removed",
                    LogEndKind::Error => "error",
                },
                code,
                message,
            },
        })
    }
}

/// Where a stream's events go; `false` means nobody is listening any more.
pub(crate) trait LogEventSink: Send + Sync + 'static {
    fn deliver(&self, event: LogStreamEvent) -> bool;
}

impl LogEventSink for Channel<LogStreamEvent> {
    fn deliver(&self, event: LogStreamEvent) -> bool {
        self.send(event).is_ok()
    }
}

struct OpenStream {
    cancellation: CancellationToken,
    retry: Arc<Notify>,
}

/// Open streams by id, in Tauri state.
#[derive(Default)]
pub(crate) struct LogStreams {
    open: Arc<Mutex<HashMap<Uuid, OpenStream>>>,
}

fn lock(open: &Mutex<HashMap<Uuid, OpenStream>>) -> MutexGuard<'_, HashMap<Uuid, OpenStream>> {
    open.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl LogStreams {
    /// Start following `request` through `connector`, delivering to `sink`; returns the id.
    pub(crate) fn open<C, S>(&self, connector: C, request: LogOpenRequest, sink: S) -> Uuid
    where
        C: LogConnector + 'static,
        S: LogEventSink,
    {
        let id = Uuid::new_v4();
        let cancellation = CancellationToken::new();
        let follower = LogFollower::new(connector, request);
        lock(&self.open).insert(
            id,
            OpenStream {
                cancellation: cancellation.clone(),
                retry: follower.retry_handle(),
            },
        );
        let open = self.open.clone();
        tokio::spawn(async move {
            run(follower, sink, cancellation).await;
            lock(&open).remove(&id);
        });
        id
    }

    /// Stop one stream; `false` when it had already ended.
    pub(crate) fn close(&self, id: Uuid) -> bool {
        let removed = lock(&self.open).remove(&id);
        removed.map(|stream| stream.cancellation.cancel()).is_some()
    }

    /// Skip a reconnecting stream's backoff.
    pub(crate) fn retry_now(&self, id: Uuid) -> bool {
        lock(&self.open)
            .get(&id)
            .map(|stream| stream.retry.notify_one())
            .is_some()
    }

    /// Logout and window close: every stream stops.
    pub(crate) fn cancel_all(&self) {
        for (_, stream) in lock(&self.open).drain() {
            stream.cancellation.cancel();
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        lock(&self.open).len()
    }
}

/// Follow until the stream ends, is cancelled, or the sink goes away; lines leave in batches.
async fn run<C: LogConnector, S: LogEventSink>(
    mut follower: LogFollower<C>,
    sink: S,
    cancellation: CancellationToken,
) {
    let (events, mut received) = mpsc::unbounded_channel();
    let follow = async {
        while let Some(event) = follower.next(&cancellation).await {
            if events.send(event).is_err() {
                break;
            }
        }
        drop(events);
    };
    let deliver = async {
        let mut lines: Vec<LogLineView> = Vec::new();
        let mut due = tokio::time::Instant::now();
        let flush = |lines: &mut Vec<LogLineView>| {
            lines.is_empty()
                || sink.deliver(LogStreamEvent::Lines {
                    lines: std::mem::take(lines),
                })
        };
        loop {
            let next = if lines.is_empty() {
                received.recv().await
            } else {
                match tokio::time::timeout_at(due, received.recv()).await {
                    Ok(next) => next,
                    Err(_) => {
                        if !flush(&mut lines) {
                            break;
                        }
                        continue;
                    }
                }
            };
            let delivered = match next {
                None => {
                    flush(&mut lines);
                    return;
                }
                Some(LogEvent::Line(line)) => {
                    if lines.is_empty() {
                        due = tokio::time::Instant::now() + LINE_BATCH_WINDOW;
                    }
                    lines.push(line.into());
                    lines.len() < MAX_BATCH_LINES || flush(&mut lines)
                }
                Some(other) => {
                    flush(&mut lines)
                        && LogStreamEvent::from_event(other).is_none_or(|event| sink.deliver(event))
                }
            };
            if !delivered {
                break;
            }
        }
        // The window is gone: stop following.
        cancellation.cancel();
    };
    tokio::join!(follow, deliver);
}

/// Connects through the app's session manager, which owns the credential and its refresh.
pub(crate) struct SessionLogConnector {
    pub(crate) app: AppHandle,
    pub(crate) profile_id: Uuid,
}

impl LogConnector for SessionLogConnector {
    fn connect<'a>(
        &'a mut self,
        request: LogOpenRequest,
        _cancellation: &'a CancellationToken,
    ) -> LogConnectFuture<'a> {
        Box::pin(async move {
            let manager = self.app.state::<SessionManager>();
            manager
                .connect_logs(self.profile_id, request)
                .await
                .map_err(log_connect_error)
        })
    }
}
