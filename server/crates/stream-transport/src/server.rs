use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use axum::{
    extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
    response::Response,
};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use kontainer_stream_protocol::{StreamFrame, WEBSOCKET_SUBPROTOCOL};
use tokio::{
    sync::{Notify, OwnedSemaphorePermit, Semaphore, mpsc, watch},
    task::JoinHandle,
    time,
};
use tokio_util::sync::CancellationToken;

use crate::{
    OutboundFrameError, StreamApplication, StreamClose, StreamLimits,
    application::{QueuedFrame, SessionSignal, outbound_queue},
};

#[derive(Clone)]
pub struct StreamServer {
    inner: Arc<ServerInner>,
}

struct ServerInner {
    application: Arc<dyn StreamApplication>,
    limits: StreamLimits,
    session_permits: Arc<Semaphore>,
    shutdown: CancellationToken,
    idle: Notify,
    active_sessions: AtomicUsize,
    started_tasks: AtomicUsize,
    completed_tasks: AtomicUsize,
    active_tasks: AtomicUsize,
    failed_task_joins: AtomicUsize,
    observed_peer_closes: AtomicUsize,
}

impl StreamServer {
    #[must_use]
    pub fn new(limits: StreamLimits, application: Arc<dyn StreamApplication>) -> Self {
        Self {
            inner: Arc::new(ServerInner {
                application,
                limits,
                session_permits: Arc::new(Semaphore::new(limits.maximum_concurrent_sessions())),
                shutdown: CancellationToken::new(),
                idle: Notify::new(),
                active_sessions: AtomicUsize::new(0),
                started_tasks: AtomicUsize::new(0),
                completed_tasks: AtomicUsize::new(0),
                active_tasks: AtomicUsize::new(0),
                failed_task_joins: AtomicUsize::new(0),
                observed_peer_closes: AtomicUsize::new(0),
            }),
        }
    }

    pub fn admit(
        &self,
        websocket: &WebSocketUpgrade,
    ) -> Result<StreamAdmission, StreamAdmissionError> {
        if self.inner.shutdown.is_cancelled() {
            return Err(StreamAdmissionError::ShuttingDown);
        }
        let supports_protocol = websocket
            .requested_protocols()
            .any(|protocol| protocol.as_bytes() == WEBSOCKET_SUBPROTOCOL.as_bytes());
        if !supports_protocol {
            return Err(StreamAdmissionError::UnsupportedSubprotocol);
        }
        let session_permit = self
            .inner
            .session_permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| StreamAdmissionError::AtCapacity)?;
        let session_registration = self.inner.register_session();
        // Shutdown may finish between the first check and this registration.
        // Recheck before handing the caller an admission that can start tasks.
        if self.inner.shutdown.is_cancelled() {
            return Err(StreamAdmissionError::ShuttingDown);
        }

        Ok(StreamAdmission {
            server: self.clone(),
            session_permit,
            session_registration,
        })
    }

    #[must_use]
    pub fn active_session_count(&self) -> usize {
        self.inner.active_sessions.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn task_counts(&self) -> StreamTaskCounts {
        StreamTaskCounts {
            started: self.inner.started_tasks.load(Ordering::Acquire),
            completed: self.inner.completed_tasks.load(Ordering::Acquire),
            active: self.inner.active_tasks.load(Ordering::Acquire),
            failed_joins: self.inner.failed_task_joins.load(Ordering::Acquire),
        }
    }

    #[must_use]
    pub fn observed_peer_close_count(&self) -> usize {
        self.inner.observed_peer_closes.load(Ordering::Acquire)
    }

    pub async fn shutdown(&self) -> Result<(), StreamShutdownError> {
        self.inner.shutdown.cancel();
        let deadline = time::Instant::now()
            .checked_add(self.inner.limits.shutdown_grace())
            .ok_or(StreamShutdownError::GraceExpired)?;

        loop {
            let notified = self.inner.idle.notified();
            if self.active_session_count() == 0 && self.task_counts().active == 0 {
                break;
            }
            if time::timeout_at(deadline, notified).await.is_err() {
                return Err(StreamShutdownError::GraceExpired);
            }
        }

        let counts = self.task_counts();
        if counts.failed_joins != 0 || counts.started != counts.completed {
            return Err(StreamShutdownError::OwnedTaskDidNotJoin);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamTaskCounts {
    pub started: usize,
    pub completed: usize,
    pub active: usize,
    pub failed_joins: usize,
}

pub struct StreamAdmission {
    server: StreamServer,
    session_permit: OwnedSemaphorePermit,
    session_registration: SessionRegistration,
}

impl StreamAdmission {
    #[must_use]
    pub fn upgrade(self, websocket: WebSocketUpgrade) -> Response {
        let application = self.server.inner.application.clone();
        self.upgrade_with(websocket, application)
    }

    /// Upgrade with an application for this connection only, e.g. one that carries the
    /// authenticated session it serves. It is dropped when the connection ends.
    #[must_use]
    pub fn upgrade_with(
        self,
        websocket: WebSocketUpgrade,
        application: Arc<dyn StreamApplication>,
    ) -> Response {
        let limits = self.server.inner.limits;
        websocket
            .protocols([WEBSOCKET_SUBPROTOCOL])
            .max_message_size(limits.maximum_frame_bytes())
            .max_frame_size(limits.maximum_frame_bytes())
            .write_buffer_size(0)
            .max_write_buffer_size(limits.maximum_queued_bytes())
            .on_upgrade(move |socket| async move {
                run_connection(
                    socket,
                    self.server,
                    application,
                    self.session_permit,
                    self.session_registration,
                )
                .await;
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamAdmissionError {
    UnsupportedSubprotocol,
    AtCapacity,
    ShuttingDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamShutdownError {
    GraceExpired,
    OwnedTaskDidNotJoin,
}

async fn run_connection(
    socket: WebSocket,
    server: StreamServer,
    application: Arc<dyn StreamApplication>,
    _session_permit: OwnedSemaphorePermit,
    _session_registration: SessionRegistration,
) {
    let (sink, stream) = socket.split();
    let (signal_sender, signal_receiver) = watch::channel(SessionSignal::Running);
    let (outbound, receiver) = outbound_queue(server.inner.limits, signal_sender.clone());
    let stop_reader = CancellationToken::new();

    let reader_server = server.clone();
    let reader_signal = signal_sender.clone();
    let reader_stop = stop_reader.clone();
    let reader_registration = server.inner.register_task();
    let mut reader = tokio::spawn(async move {
        read_loop(
            stream,
            outbound,
            application,
            reader_server,
            reader_signal,
            reader_stop,
        )
        .await;
        reader_registration.complete();
    });

    let writer_server = server.clone();
    let writer_registration = server.inner.register_task();
    let mut writer = tokio::spawn(async move {
        write_loop(sink, receiver, signal_receiver, writer_server).await;
        writer_registration.complete();
    });

    enum FirstTask {
        Reader(Result<(), tokio::task::JoinError>),
        Writer(Result<(), tokio::task::JoinError>),
    }

    let first = tokio::select! {
        result = &mut reader => FirstTask::Reader(result),
        result = &mut writer => FirstTask::Writer(result),
    };
    match first {
        FirstTask::Reader(result) => {
            record_join_result(&server.inner, result);
            await_owned_task(&server.inner, &mut writer).await;
        }
        FirstTask::Writer(result) => {
            record_join_result(&server.inner, result);
            stop_reader.cancel();
            await_owned_task(&server.inner, &mut reader).await;
        }
    }
}

async fn await_owned_task(inner: &Arc<ServerInner>, task: &mut JoinHandle<()>) {
    match time::timeout(inner.limits.shutdown_grace(), &mut *task).await {
        Ok(result) => record_join_result(inner, result),
        Err(_) => {
            task.abort();
            let _ = task.await;
            inner.failed_task_joins.fetch_add(1, Ordering::AcqRel);
            inner.idle.notify_waiters();
        }
    }
}

fn record_join_result(inner: &Arc<ServerInner>, result: Result<(), tokio::task::JoinError>) {
    if result.is_err() {
        inner.failed_task_joins.fetch_add(1, Ordering::AcqRel);
        inner.idle.notify_waiters();
    }
}

async fn read_loop(
    mut stream: futures_util::stream::SplitStream<WebSocket>,
    outbound: crate::OutboundSender,
    application: Arc<dyn StreamApplication>,
    server: StreamServer,
    signal: watch::Sender<SessionSignal>,
    stop: CancellationToken,
) {
    loop {
        let message = tokio::select! {
            biased;
            () = server.inner.shutdown.cancelled() => {
                signal_close(&signal, StreamClose::SERVER_SHUTDOWN);
                return;
            }
            () = stop.cancelled() => return,
            message = stream.next() => message,
        };

        match message {
            Some(Ok(Message::Binary(bytes))) => {
                if bytes.len() > server.inner.limits.maximum_frame_bytes() {
                    signal_close(&signal, StreamClose::MESSAGE_TOO_LARGE);
                    return;
                }
                let frame = match StreamFrame::decode(&bytes) {
                    Ok(frame) => frame,
                    Err(error) => {
                        signal_close(&signal, StreamClose::for_envelope_error(error));
                        return;
                    }
                };

                let application = application.handle(frame, &outbound);
                let result = tokio::select! {
                    biased;
                    () = server.inner.shutdown.cancelled() => {
                        signal_close(&signal, StreamClose::SERVER_SHUTDOWN);
                        return;
                    }
                    () = stop.cancelled() => return,
                    result = application => result,
                };
                match result {
                    Ok(()) => {}
                    Err(OutboundFrameError::QueueOverloaded) => {
                        signal_close(&signal, StreamClose::SLOW_CONSUMER);
                        return;
                    }
                    Err(OutboundFrameError::FrameTooLarge | OutboundFrameError::SessionClosed) => {
                        signal_close(&signal, StreamClose::APPLICATION_FAILURE);
                        return;
                    }
                }
            }
            Some(Ok(Message::Text(_))) => {
                signal_close(&signal, StreamClose::BINARY_REQUIRED);
                return;
            }
            Some(Ok(Message::Close(_))) => {
                server
                    .inner
                    .observed_peer_closes
                    .fetch_add(1, Ordering::AcqRel);
                let _ = signal.send(SessionSignal::PeerClosed);
                return;
            }
            None => {
                let _ = signal.send(SessionSignal::PeerClosed);
                return;
            }
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
            Some(Err(error)) => {
                let error = error.into_inner();
                let close = error
                    .downcast_ref::<tokio_tungstenite::tungstenite::Error>()
                    .and_then(StreamClose::for_transport_error);
                if let Some(close) = close {
                    signal_close(&signal, close);
                } else {
                    let _ = signal.send(SessionSignal::PeerClosed);
                }
                return;
            }
        }
    }
}

async fn write_loop(
    mut sink: SplitSink<WebSocket, Message>,
    mut receiver: mpsc::Receiver<QueuedFrame>,
    mut signal: watch::Receiver<SessionSignal>,
    server: StreamServer,
) {
    // A quiet connection still carries a Ping, so the client can tell quiet from dead.
    let every = server.inner.limits.ping_interval();
    let mut ping = time::interval_at(time::Instant::now() + every, every);
    loop {
        tokio::select! {
            biased;
            () = server.inner.shutdown.cancelled() => {
                send_close(&mut sink, StreamClose::SERVER_SHUTDOWN, server.inner.limits).await;
                return;
            }
            changed = signal.changed() => {
                if changed.is_err() {
                    return;
                }
                let current = *signal.borrow_and_update();
                match current {
                    SessionSignal::Running => {}
                    SessionSignal::Close(close) => {
                        send_close(&mut sink, close, server.inner.limits).await;
                        return;
                    }
                    SessionSignal::Finish(close) => {
                        let limits = server.inner.limits;
                        let _ = time::timeout(limits.shutdown_grace(), async {
                            while let Ok(queued) = receiver.try_recv() {
                                if sink.send(Message::Binary(queued.encoded.into())).await.is_err() {
                                    return;
                                }
                            }
                        })
                        .await;
                        send_close(&mut sink, close, limits).await;
                        return;
                    }
                    SessionSignal::PeerClosed => {
                        let _ = time::timeout(
                            server.inner.limits.shutdown_grace(),
                            sink.flush(),
                        )
                        .await;
                        return;
                    }
                }
            }
            queued = receiver.recv() => match queued {
                Some(queued) => {
                    if sink.send(Message::Binary(queued.encoded.into())).await.is_err() {
                        return;
                    }
                }
                None => return,
            },
            _ = ping.tick() => {
                if sink.send(Message::Ping(axum::body::Bytes::new())).await.is_err() {
                    return;
                }
            }
        }
    }
}

fn signal_close(signal: &watch::Sender<SessionSignal>, close: StreamClose) {
    signal.send_if_modified(|current| {
        if *current == SessionSignal::Running {
            *current = SessionSignal::Close(close);
            true
        } else {
            false
        }
    });
}

async fn send_close(
    sink: &mut SplitSink<WebSocket, Message>,
    close: StreamClose,
    limits: StreamLimits,
) {
    let message = Message::Close(Some(CloseFrame {
        code: close.code,
        reason: close.reason.into(),
    }));
    let _ = time::timeout(limits.shutdown_grace(), sink.send(message)).await;
}

impl ServerInner {
    fn register_session(self: &Arc<Self>) -> SessionRegistration {
        self.active_sessions.fetch_add(1, Ordering::AcqRel);
        SessionRegistration {
            inner: self.clone(),
        }
    }

    fn register_task(self: &Arc<Self>) -> TaskRegistration {
        self.started_tasks.fetch_add(1, Ordering::AcqRel);
        self.active_tasks.fetch_add(1, Ordering::AcqRel);
        TaskRegistration {
            inner: self.clone(),
        }
    }
}

struct SessionRegistration {
    inner: Arc<ServerInner>,
}

impl Drop for SessionRegistration {
    fn drop(&mut self) {
        self.inner.active_sessions.fetch_sub(1, Ordering::AcqRel);
        self.inner.idle.notify_waiters();
    }
}

struct TaskRegistration {
    inner: Arc<ServerInner>,
}

impl TaskRegistration {
    fn complete(self) {
        self.inner.completed_tasks.fetch_add(1, Ordering::AcqRel);
    }
}

impl Drop for TaskRegistration {
    fn drop(&mut self) {
        self.inner.active_tasks.fetch_sub(1, Ordering::AcqRel);
        self.inner.idle.notify_waiters();
    }
}
