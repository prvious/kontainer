//! Open exec sessions (ADR 0006, ADR 0026): each reads its shell's events in its own task and
//! pushes them to React over a Channel; input, resizes, and the close arrive as commands. The
//! registry maps session ids to their writer and cancellation; logout and window close end all.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use base64::Engine as _;
use kontainer_native_client::{
    ExecConnection, ExecEvent, ExecEventSource, ExecInputFrame, ExecInputSink,
};
use serde::Serialize;
use tauri::ipc::Channel;
use tokio::sync::Mutex as AsyncMutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// What a session's Channel carries; mirrors `ExecStreamEvent` in `app/src/types/exec.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum ExecStreamEvent {
    Started {
        shell: String,
    },
    /// Base64 of the raw bytes the terminal wrote.
    Output {
        data: String,
    },
    Exit {
        code: Option<i64>,
        reason: String,
    },
    Error {
        code: String,
        message: String,
    },
    Closed {
        reason: &'static str,
    },
}

impl From<ExecEvent> for ExecStreamEvent {
    fn from(event: ExecEvent) -> Self {
        match event {
            ExecEvent::Started { shell, .. } => Self::Started { shell },
            ExecEvent::Output(bytes) => Self::Output {
                data: base64::engine::general_purpose::STANDARD.encode(bytes),
            },
            ExecEvent::Exit { code, reason } => Self::Exit { code, reason },
            ExecEvent::Error { code, message } => Self::Error { code, message },
            ExecEvent::Closed { reason } => Self::Closed {
                reason: reason.as_str(),
            },
        }
    }
}

/// Where a session's events go; `false` means nobody is listening any more.
pub(crate) trait ExecEventSink: Send + Sync + 'static {
    fn deliver(&self, event: ExecStreamEvent) -> bool;
}

impl ExecEventSink for Channel<ExecStreamEvent> {
    fn deliver(&self, event: ExecStreamEvent) -> bool {
        self.send(event).is_ok()
    }
}

type Writer = Arc<AsyncMutex<Box<dyn ExecInputSink>>>;

struct OpenSession {
    writer: Writer,
    cancellation: CancellationToken,
}

/// Open sessions by id, in Tauri state.
#[derive(Default)]
pub(crate) struct ExecSessions {
    open: Arc<Mutex<HashMap<Uuid, OpenSession>>>,
}

fn lock(open: &Mutex<HashMap<Uuid, OpenSession>>) -> MutexGuard<'_, HashMap<Uuid, OpenSession>> {
    open.lock().unwrap_or_else(PoisonError::into_inner)
}

impl ExecSessions {
    /// Start relaying `connection`'s events to `sink`; returns the session id.
    pub(crate) fn open<S: ExecEventSink>(&self, connection: ExecConnection, sink: S) -> Uuid {
        let id = Uuid::new_v4();
        let cancellation = CancellationToken::new();
        let ExecConnection { input, events } = connection;
        lock(&self.open).insert(
            id,
            OpenSession {
                writer: Arc::new(AsyncMutex::new(input)),
                cancellation: cancellation.clone(),
            },
        );
        let open = self.open.clone();
        tokio::spawn(async move {
            relay(events, &sink, &cancellation).await;
            lock(&open).remove(&id);
        });
        id
    }

    fn writer(&self, id: Uuid) -> Option<(Writer, CancellationToken)> {
        lock(&self.open)
            .get(&id)
            .map(|session| (session.writer.clone(), session.cancellation.clone()))
    }

    /// Send to one session; `false` when it has already ended.
    pub(crate) async fn send(&self, id: Uuid, frame: ExecInputFrame) -> bool {
        let Some((writer, cancellation)) = self.writer(id) else {
            return false;
        };
        let mut writer = writer.lock().await;
        writer.send(frame, &cancellation).await.is_ok()
    }

    /// Ask the shell to exit; its exit and the close still arrive as events.
    pub(crate) async fn close(&self, id: Uuid) -> bool {
        self.send(id, ExecInputFrame::Close).await
    }

    /// Logout and window close: every session ends now.
    pub(crate) fn cancel_all(&self) {
        for (_, session) in lock(&self.open).drain() {
            session.cancellation.cancel();
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        lock(&self.open).len()
    }
}

async fn relay<S: ExecEventSink>(
    mut events: Box<dyn ExecEventSource>,
    sink: &S,
    cancellation: &CancellationToken,
) {
    while let Some(event) = events.next_event(cancellation).await {
        let last = matches!(event, ExecEvent::Closed { .. });
        if !sink.deliver(event.into()) || last {
            break;
        }
    }
    cancellation.cancel();
}
