use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::{
    Router,
    extract::{
        State,
        ws::{Message as AxumMessage, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use kontainer_stream_protocol::{StreamFrame, StreamFrameKind, WEBSOCKET_SUBPROTOCOL};
use kontainer_stream_transport::{
    NativeStreamClient, OutboundFrameError, OutboundSender, StreamAdmissionError,
    StreamApplication, StreamClientError, StreamClose, StreamLimits, StreamServer,
};
use tokio::{
    net::TcpListener,
    sync::{Mutex, mpsc, oneshot},
    task::JoinHandle,
    time,
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, connect_async_with_config,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        http::{HeaderValue, header::SEC_WEBSOCKET_PROTOCOL},
        protocol::WebSocketConfig,
    },
};
use tokio_util::sync::CancellationToken;

type RawSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

#[test]
fn qualification_manifest_records_fixtures_limits_and_close_contract() {
    let limits = StreamLimits::default();
    let fixture = StreamFrame::data(7, b"ping".to_vec()).encode();

    println!(
        "KONTAINER_STREAM_EVIDENCE architecture={} subprotocol={} fixture={fixture:?} \
         max_frame_bytes={} max_queue_frames={} max_queue_bytes={} max_sessions={} \
         shutdown_grace_ms={} text_close={}:{} version_close={}:{} malformed_close={}:{} \
         oversized_close={}:{} overload_close={}:{}",
        std::env::consts::ARCH,
        WEBSOCKET_SUBPROTOCOL,
        limits.maximum_frame_bytes(),
        limits.maximum_queued_frames(),
        limits.maximum_queued_bytes(),
        limits.maximum_concurrent_sessions(),
        limits.shutdown_grace().as_millis(),
        StreamClose::BINARY_REQUIRED.code,
        StreamClose::BINARY_REQUIRED.reason,
        StreamClose::UNSUPPORTED_VERSION.code,
        StreamClose::UNSUPPORTED_VERSION.reason,
        StreamClose::MALFORMED_ENVELOPE.code,
        StreamClose::MALFORMED_ENVELOPE.reason,
        StreamClose::MESSAGE_TOO_LARGE.code,
        StreamClose::MESSAGE_TOO_LARGE.reason,
        StreamClose::SLOW_CONSUMER.code,
        StreamClose::SLOW_CONSUMER.reason,
    );
}

struct AcknowledgingApplication;

#[async_trait]
impl StreamApplication for AcknowledgingApplication {
    async fn handle(
        &self,
        frame: StreamFrame,
        outbound: &OutboundSender,
    ) -> Result<(), OutboundFrameError> {
        outbound.try_send(StreamFrame::acknowledgement(frame.sequence, frame.payload))
    }
}

struct SilentApplication;

#[async_trait]
impl StreamApplication for SilentApplication {
    async fn handle(
        &self,
        _frame: StreamFrame,
        _outbound: &OutboundSender,
    ) -> Result<(), OutboundFrameError> {
        Ok(())
    }
}

struct BurstApplication;

#[async_trait]
impl StreamApplication for BurstApplication {
    async fn handle(
        &self,
        frame: StreamFrame,
        outbound: &OutboundSender,
    ) -> Result<(), OutboundFrameError> {
        for offset in 0..3 {
            outbound.try_send(StreamFrame::acknowledgement(
                frame.sequence + offset,
                vec![b'x'; 96],
            ))?;
        }
        Ok(())
    }
}

struct FloodingApplication {
    overload: mpsc::UnboundedSender<usize>,
}

#[async_trait]
impl StreamApplication for FloodingApplication {
    async fn handle(
        &self,
        frame: StreamFrame,
        outbound: &OutboundSender,
    ) -> Result<(), OutboundFrameError> {
        let payload = vec![b's'; 60 * 1024];

        for offset in 0..4_096 {
            match outbound.try_send(StreamFrame::acknowledgement(
                frame.sequence + offset,
                payload.clone(),
            )) {
                Ok(()) => time::sleep(Duration::from_millis(1)).await,
                Err(error @ OutboundFrameError::QueueOverloaded) => {
                    let _ = self.overload.send(offset as usize);
                    return Err(error);
                }
                Err(error) => return Err(error),
            }
        }

        panic!("non-reading peer accepted the entire bounded flood")
    }
}

#[tokio::test]
async fn binary_frames_round_trip_through_native_client_and_axum_server() {
    let running =
        LoopbackServer::start(StreamLimits::default(), Arc::new(AcknowledgingApplication)).await;
    let cancellation = CancellationToken::new();
    let mut client =
        NativeStreamClient::connect(&running.endpoint, StreamLimits::default(), &cancellation)
            .await
            .unwrap();

    let acknowledgement = client
        .round_trip(
            &StreamFrame::data(41, b"echo-fixture".to_vec()),
            &cancellation,
        )
        .await
        .unwrap();

    assert_eq!(acknowledgement.kind, StreamFrameKind::Acknowledgement);
    assert_eq!(acknowledgement.sequence, 41);
    assert_eq!(acknowledgement.payload, b"echo-fixture");
    client.close().await;
    running.finish().await;
}

#[tokio::test]
async fn text_unknown_version_unknown_kind_and_malformed_frames_close_explicitly() {
    assert_invalid_frame(
        Message::Text("not protobuf".into()),
        StreamClose::BINARY_REQUIRED,
    )
    .await;

    let mut unknown_version = StreamFrame::data(1, Vec::new()).encode();
    unknown_version[1] = 2;
    assert_invalid_frame(
        Message::Binary(unknown_version.into()),
        StreamClose::UNSUPPORTED_VERSION,
    )
    .await;

    let mut unknown_kind = StreamFrame::data(1, Vec::new()).encode();
    let kind_value = unknown_kind
        .iter()
        .position(|byte| *byte == 24)
        .expect("fixture contains kind field")
        + 1;
    unknown_kind[kind_value] = 99;
    assert_invalid_frame(
        Message::Binary(unknown_kind.into()),
        StreamClose::UNKNOWN_FRAME_KIND,
    )
    .await;

    assert_invalid_frame(
        Message::Binary(vec![0xff].into()),
        StreamClose::MALFORMED_ENVELOPE,
    )
    .await;
}

#[tokio::test]
async fn oversized_frames_close_with_the_configured_code_and_reason() {
    let limits = StreamLimits::new(128, 4, 1024, 1, Duration::from_secs(1)).unwrap();
    let running = LoopbackServer::start(limits, Arc::new(AcknowledgingApplication)).await;
    let mut socket = connect_raw(&running.endpoint, 1024).await;

    socket
        .send(Message::Binary(vec![0; 129].into()))
        .await
        .unwrap();
    assert_eq!(
        next_close(&mut socket).await,
        StreamClose::MESSAGE_TOO_LARGE
    );

    running.finish().await;
}

#[tokio::test]
async fn missing_subprotocol_and_excess_concurrency_fail_before_upgrade() {
    let running = LoopbackServer::start(
        StreamLimits::new(1024, 4, 4096, 1, Duration::from_secs(1)).unwrap(),
        Arc::new(SilentApplication),
    )
    .await;

    let missing_protocol = connect_async(&running.endpoint).await.unwrap_err();
    assert_http_status(missing_protocol, StatusCode::BAD_REQUEST);

    let cancellation = CancellationToken::new();
    let mut admitted =
        NativeStreamClient::connect(&running.endpoint, StreamLimits::default(), &cancellation)
            .await
            .unwrap();
    wait_for_active_sessions(&running.server, 1).await;

    let rejected =
        NativeStreamClient::connect(&running.endpoint, StreamLimits::default(), &cancellation)
            .await
            .err()
            .unwrap();
    assert_eq!(
        rejected,
        StreamClientError::HandshakeRejected {
            status: StatusCode::TOO_MANY_REQUESTS.as_u16()
        }
    );

    admitted.close().await;
    running.finish().await;
}

#[tokio::test(flavor = "current_thread")]
async fn synchronous_burst_cannot_exceed_the_bounded_queue() {
    let limits = StreamLimits::new(256, 1, 256, 1, Duration::from_secs(1)).unwrap();
    let running = LoopbackServer::start(limits, Arc::new(BurstApplication)).await;
    let mut socket = connect_raw(&running.endpoint, 1024).await;

    socket
        .send(Message::Binary(
            StreamFrame::data(5, b"burst".to_vec()).encode().into(),
        ))
        .await
        .unwrap();

    assert_eq!(next_close(&mut socket).await, StreamClose::SLOW_CONSUMER);
    running.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn non_reading_peer_eventually_hits_slow_consumer_backpressure() {
    let limits =
        StreamLimits::new(64 * 1024, 32, 2 * 1024 * 1024, 1, Duration::from_secs(2)).unwrap();
    let (overload, mut overload_observed) = mpsc::unbounded_channel();
    let running = LoopbackServer::start(limits, Arc::new(FloodingApplication { overload })).await;
    let mut socket = connect_raw(&running.endpoint, 64 * 1024).await;

    socket
        .send(Message::Binary(
            StreamFrame::data(1, b"start outbound flood".to_vec())
                .encode()
                .into(),
        ))
        .await
        .unwrap();

    let sent_frames = time::timeout(Duration::from_secs(5), overload_observed.recv())
        .await
        .expect("non-reading peer eventually backs up the server writer")
        .expect("flood application reports its bounded-queue overload");
    assert!(sent_frames > limits.maximum_queued_frames());
    println!(
        "KONTAINER_STREAM_SLOW_CONSUMER_EVIDENCE sent_without_reading={sent_frames} \
         queue_frames={} queue_bytes={}",
        limits.maximum_queued_frames(),
        limits.maximum_queued_bytes()
    );
    assert_eq!(next_close(&mut socket).await, StreamClose::SLOW_CONSUMER);
    running.finish().await;
}

#[tokio::test]
async fn client_cancellation_closes_the_peer_and_all_owned_tasks_join() {
    let running = LoopbackServer::start(StreamLimits::default(), Arc::new(SilentApplication)).await;
    let cancellation = CancellationToken::new();
    let mut client =
        NativeStreamClient::connect(&running.endpoint, StreamLimits::default(), &cancellation)
            .await
            .unwrap();
    client
        .send_frame(&StreamFrame::data(1, b"wait".to_vec()), &cancellation)
        .await
        .unwrap();

    cancellation.cancel();
    assert_eq!(
        client.receive_frame(&cancellation).await,
        Err(StreamClientError::Cancelled)
    );

    wait_for_drained_session(&running.server).await;
    assert_eq!(running.server.observed_peer_close_count(), 1);
    let counts = running.server.task_counts();
    assert_eq!(counts.started, counts.completed);
    assert_eq!(counts.active, 0);
    assert_eq!(counts.failed_joins, 0);

    running.finish().await;
}

#[tokio::test]
async fn native_client_rejects_invalid_server_frames_with_owned_closes() {
    assert_native_rejection(
        AxumMessage::Text("not protobuf".into()),
        StreamLimits::default(),
        StreamClientError::UnexpectedTextFrame,
        StreamClose::BINARY_REQUIRED,
    )
    .await;

    let mut unknown_version = StreamFrame::data(1, Vec::new()).encode();
    unknown_version[1] = 2;
    assert_native_rejection(
        AxumMessage::Binary(unknown_version.into()),
        StreamLimits::default(),
        StreamClientError::Protocol(
            kontainer_stream_protocol::StreamProtocolError::UnsupportedProtocolVersion(2),
        ),
        StreamClose::UNSUPPORTED_VERSION,
    )
    .await;

    let mut unknown_kind = StreamFrame::data(1, Vec::new()).encode();
    let kind_value = unknown_kind
        .iter()
        .position(|byte| *byte == 24)
        .expect("fixture contains kind field")
        + 1;
    unknown_kind[kind_value] = 99;
    assert_native_rejection(
        AxumMessage::Binary(unknown_kind.into()),
        StreamLimits::default(),
        StreamClientError::Protocol(
            kontainer_stream_protocol::StreamProtocolError::UnknownFrameKind(99),
        ),
        StreamClose::UNKNOWN_FRAME_KIND,
    )
    .await;

    assert_native_rejection(
        AxumMessage::Binary(vec![0xff].into()),
        StreamLimits::default(),
        StreamClientError::Protocol(
            kontainer_stream_protocol::StreamProtocolError::MalformedEnvelope,
        ),
        StreamClose::MALFORMED_ENVELOPE,
    )
    .await;

    assert_native_rejection(
        AxumMessage::Binary(vec![0; 129].into()),
        StreamLimits::new(128, 4, 1024, 1, Duration::from_secs(1)).unwrap(),
        StreamClientError::FrameTooLarge,
        StreamClose::MESSAGE_TOO_LARGE,
    )
    .await;
}

#[tokio::test]
async fn server_cancellation_closes_the_client_within_grace_and_joins_every_task() {
    let limits = StreamLimits::new(1024, 4, 4096, 1, Duration::from_secs(1)).unwrap();
    let running = LoopbackServer::start(limits, Arc::new(SilentApplication)).await;
    let cancellation = CancellationToken::new();
    let mut client =
        NativeStreamClient::connect(&running.endpoint, StreamLimits::default(), &cancellation)
            .await
            .unwrap();
    wait_for_active_sessions(&running.server, 1).await;

    time::timeout(Duration::from_secs(2), running.server.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        client.receive_frame(&cancellation).await,
        Err(StreamClientError::Closed {
            code: StreamClose::SERVER_SHUTDOWN.code,
            reason: StreamClose::SERVER_SHUTDOWN.reason.to_owned(),
        })
    );

    running.finish_after_stream_shutdown().await;
}

async fn assert_invalid_frame(message: Message, expected: StreamClose) {
    let running = LoopbackServer::start(StreamLimits::default(), Arc::new(SilentApplication)).await;
    let mut socket = connect_raw(&running.endpoint, 128 * 1024).await;
    socket.send(message).await.unwrap();

    assert_eq!(next_close(&mut socket).await, expected);
    running.finish().await;
}

async fn assert_native_rejection(
    message: AxumMessage,
    limits: StreamLimits,
    expected_error: StreamClientError,
    expected_close: StreamClose,
) {
    let invalid_server = InvalidLoopbackServer::start(message).await;
    let cancellation = CancellationToken::new();
    let mut client = NativeStreamClient::connect(&invalid_server.endpoint, limits, &cancellation)
        .await
        .unwrap();

    assert_eq!(
        client.receive_frame(&cancellation).await,
        Err(expected_error)
    );
    invalid_server.finish(expected_close).await;
}

async fn connect_raw(endpoint: &str, maximum_frame_bytes: usize) -> RawSocket {
    let mut request = endpoint.into_client_request().unwrap();
    request.headers_mut().insert(
        SEC_WEBSOCKET_PROTOCOL,
        HeaderValue::from_static(WEBSOCKET_SUBPROTOCOL),
    );
    let configuration = WebSocketConfig::default()
        .write_buffer_size(0)
        .max_write_buffer_size(maximum_frame_bytes * 2)
        .max_message_size(Some(maximum_frame_bytes))
        .max_frame_size(Some(maximum_frame_bytes));

    connect_async_with_config(request, Some(configuration), false)
        .await
        .unwrap()
        .0
}

async fn next_close(socket: &mut RawSocket) -> StreamClose {
    time::timeout(Duration::from_secs(2), async {
        loop {
            match socket.next().await {
                Some(Ok(Message::Close(Some(frame)))) => {
                    return StreamClose::new(
                        u16::from(frame.code),
                        match frame.reason.as_str() {
                            "binary protobuf required" => StreamClose::BINARY_REQUIRED.reason,
                            "unsupported protocol version" => {
                                StreamClose::UNSUPPORTED_VERSION.reason
                            }
                            "unknown frame kind" => StreamClose::UNKNOWN_FRAME_KIND.reason,
                            "malformed protobuf envelope" => StreamClose::MALFORMED_ENVELOPE.reason,
                            "frame exceeds configured limit" => {
                                StreamClose::MESSAGE_TOO_LARGE.reason
                            }
                            "slow consumer exceeded queue limits" => {
                                StreamClose::SLOW_CONSUMER.reason
                            }
                            reason => panic!("unexpected close reason {reason}"),
                        },
                    );
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => panic!("WebSocket failed before close: {error}"),
                None => panic!("WebSocket ended before close"),
            }
        }
    })
    .await
    .expect("server sends close within the shutdown bound")
}

fn assert_http_status(error: tokio_tungstenite::tungstenite::Error, expected: StatusCode) {
    match error {
        tokio_tungstenite::tungstenite::Error::Http(response) => {
            assert_eq!(response.status(), expected)
        }
        other => panic!("expected HTTP handshake rejection, got {other}"),
    }
}

async fn wait_for_active_sessions(server: &StreamServer, expected: usize) {
    time::timeout(Duration::from_secs(1), async {
        loop {
            if server.active_session_count() == expected {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("session reaches expected lifecycle state");
}

async fn wait_for_drained_session(server: &StreamServer) {
    time::timeout(Duration::from_secs(2), async {
        loop {
            if server.active_session_count() == 0 && server.task_counts().active == 0 {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("peer close drains the admitted session without server shutdown");
}

struct LoopbackServer {
    endpoint: String,
    server: StreamServer,
    http_shutdown: CancellationToken,
    http_task: JoinHandle<()>,
}

struct InvalidFixture {
    message: Mutex<Option<AxumMessage>>,
    observed_close: Mutex<Option<oneshot::Sender<(u16, String)>>>,
}

struct InvalidLoopbackServer {
    endpoint: String,
    observed_close: oneshot::Receiver<(u16, String)>,
    http_shutdown: CancellationToken,
    http_task: JoinHandle<()>,
}

impl InvalidLoopbackServer {
    async fn start(message: AxumMessage) -> Self {
        let (close_sender, observed_close) = oneshot::channel();
        let fixture = Arc::new(InvalidFixture {
            message: Mutex::new(Some(message)),
            observed_close: Mutex::new(Some(close_sender)),
        });
        let router = Router::new()
            .route("/invalid", get(invalid_upgrade))
            .with_state(fixture);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let http_shutdown = CancellationToken::new();
        let shutdown = http_shutdown.clone();
        let http_task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
                .unwrap();
        });

        Self {
            endpoint: format!("ws://{address}/invalid"),
            observed_close,
            http_shutdown,
            http_task,
        }
    }

    async fn finish(self, expected: StreamClose) {
        let (code, reason) = time::timeout(Duration::from_secs(2), self.observed_close)
            .await
            .expect("native client sends its rejection close")
            .expect("invalid fixture observes the close");
        assert_eq!(code, expected.code);
        assert_eq!(reason, expected.reason);
        self.http_shutdown.cancel();
        self.http_task.await.unwrap();
    }
}

impl LoopbackServer {
    async fn start(limits: StreamLimits, application: Arc<dyn StreamApplication>) -> Self {
        let server = StreamServer::new(limits, application);
        let router = Router::new()
            .route("/api/v1/streams/qualification", get(upgrade))
            .with_state(server.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let http_shutdown = CancellationToken::new();
        let shutdown = http_shutdown.clone();
        let http_task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
                .unwrap();
        });

        Self {
            endpoint: format!("ws://{address}/api/v1/streams/qualification"),
            server,
            http_shutdown,
            http_task,
        }
    }

    async fn finish(self) {
        self.server.shutdown().await.unwrap();
        self.finish_after_stream_shutdown().await;
    }

    async fn finish_after_stream_shutdown(self) {
        self.http_shutdown.cancel();
        self.http_task.await.unwrap();
        let counts = self.server.task_counts();
        assert_eq!(self.server.active_session_count(), 0);
        assert_eq!(counts.active, 0);
        assert_eq!(counts.failed_joins, 0);
        assert_eq!(counts.started, counts.completed);
        assert!(counts.started == 0 || counts.started >= 2);
        println!(
            "KONTAINER_STREAM_TASK_EVIDENCE started={} completed={} active={} failed_joins={}",
            counts.started, counts.completed, counts.active, counts.failed_joins
        );
    }
}

async fn upgrade(State(server): State<StreamServer>, websocket: WebSocketUpgrade) -> Response {
    match server.admit(&websocket) {
        Ok(admission) => admission.upgrade(websocket),
        Err(StreamAdmissionError::UnsupportedSubprotocol) => {
            (StatusCode::BAD_REQUEST, "required stream subprotocol").into_response()
        }
        Err(StreamAdmissionError::AtCapacity) => {
            (StatusCode::TOO_MANY_REQUESTS, "stream capacity exhausted").into_response()
        }
        Err(StreamAdmissionError::ShuttingDown) => {
            (StatusCode::SERVICE_UNAVAILABLE, "stream server draining").into_response()
        }
    }
}

async fn invalid_upgrade(
    State(fixture): State<Arc<InvalidFixture>>,
    websocket: WebSocketUpgrade,
) -> Response {
    websocket
        .protocols([WEBSOCKET_SUBPROTOCOL])
        .on_upgrade(move |mut socket| async move {
            let message = fixture
                .message
                .lock()
                .await
                .take()
                .expect("fixture serves one connection");
            socket.send(message).await.unwrap();

            while let Some(message) = socket.next().await {
                if let Ok(AxumMessage::Close(Some(frame))) = message {
                    let observed = (frame.code, frame.reason.to_string());
                    let _ = socket.flush().await;
                    if let Some(sender) = fixture.observed_close.lock().await.take() {
                        let _ = sender.send(observed);
                    }
                    return;
                }
            }
        })
}
