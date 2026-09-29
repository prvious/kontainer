//! Log following: the reconnect schedule, overlap dedupe, end propagation, and one real
//! upgrade against a loopback stream server.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Router,
    extract::{State, ws::WebSocketUpgrade},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use kontainer_stream_protocol::{
    LogEnd, LogEndReason, LogLine, LogMessage, LogStreamKind, LogsOpen, StreamFrame,
};
use kontainer_stream_transport::{
    OutboundFrameError, OutboundSender, StreamApplication, StreamLimits, StreamServer,
};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::logs::{
    LogConnectError, LogConnectFuture, LogConnector, LogDisconnect, LogEndKind, LogEvent,
    LogFollower, LogFrame, LogFrameFuture, LogLineItem, LogOpenRequest, LogReceiver, LogStream,
    Overlap, reconnect_delay,
};
use crate::{AccessCredential, NativeClient, NativeClientConfig, NativeClientError, ServiceId};

const SERVICE: &str = "eb6ymskrt4ij70dco3menddxl";
const ACCESS: &str = "kta_11111111111111111111111111111111.2222222222222222222222222222222222222222222222222222222222222222";

fn item(ts: i64, text: &str) -> LogLineItem {
    LogLineItem {
        seq: 0,
        ts_unix_ms: ts,
        stream: LogStream::Stdout,
        task_id: "task".to_owned(),
        node_id: "node".to_owned(),
        text: text.to_owned(),
        truncated: false,
    }
}

fn request() -> LogOpenRequest {
    LogOpenRequest {
        service_id: ServiceId::parse(SERVICE).unwrap(),
        since_unix_ms: None,
        tail: 500,
    }
}

#[test]
fn reconnects_back_off_from_one_second_doubling_to_thirty() {
    let delays: Vec<u64> = (1..=8)
        .map(|attempt| reconnect_delay(attempt).as_secs())
        .collect();
    assert_eq!(delays, vec![1, 2, 4, 8, 16, 30, 30, 30]);
    assert_eq!(reconnect_delay(u32::MAX).as_secs(), 30);
}

#[test]
fn the_overlap_drops_repeats_and_lines_older_than_the_window() {
    let mut overlap = Overlap::default();
    assert!(overlap.accept(&item(10_000, "a")));
    assert!(overlap.accept(&item(10_500, "b")));
    assert!(!overlap.accept(&item(10_500, "b")), "the same line again");
    assert!(overlap.accept(&item(10_500, "c")), "same time, other text");
    assert!(
        !overlap.accept(&item(9_400, "old")),
        "older than one second before the last"
    );
    assert!(
        overlap.accept(&item(9_600, "late")),
        "inside the window and new"
    );
    assert_eq!(overlap.last_ts(), Some(10_500));
}

type Script = Arc<Mutex<VecDeque<Result<Vec<Result<LogFrame, LogDisconnect>>, LogConnectError>>>>;

struct ScriptedReceiver(VecDeque<Result<LogFrame, LogDisconnect>>);

impl LogReceiver for ScriptedReceiver {
    fn next_frame<'a>(&'a mut self, _cancellation: &'a CancellationToken) -> LogFrameFuture<'a> {
        let next = self.0.pop_front().unwrap_or(Err(LogDisconnect::Dropped));
        Box::pin(async move { next })
    }
}

struct ScriptedConnector {
    script: Script,
    requests: Arc<Mutex<Vec<LogOpenRequest>>>,
}

impl LogConnector for ScriptedConnector {
    fn connect<'a>(
        &'a mut self,
        request: LogOpenRequest,
        _cancellation: &'a CancellationToken,
    ) -> LogConnectFuture<'a> {
        self.requests.lock().unwrap().push(request);
        let next = self
            .script
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err(LogConnectError::Terminal("script_ended".to_owned())));
        Box::pin(async move {
            next.map(|frames| Box::new(ScriptedReceiver(frames.into())) as Box<dyn LogReceiver>)
        })
    }
}

fn follower(
    connections: Vec<Result<Vec<Result<LogFrame, LogDisconnect>>, LogConnectError>>,
) -> (
    LogFollower<ScriptedConnector>,
    Arc<Mutex<Vec<LogOpenRequest>>>,
) {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let connector = ScriptedConnector {
        script: Arc::new(Mutex::new(connections.into())),
        requests: requests.clone(),
    };
    (LogFollower::new(connector, request()), requests)
}

/// Every event until the follower ends, skipping backoff waits.
async fn drain(follower: &mut LogFollower<ScriptedConnector>) -> Vec<LogEvent> {
    let cancellation = CancellationToken::new();
    let retry = follower.retry_handle();
    let mut events = Vec::new();
    while let Some(event) =
        tokio::time::timeout(Duration::from_secs(2), follower.next(&cancellation))
            .await
            .expect("the follower never waits out a backoff here")
    {
        if matches!(event, LogEvent::Reconnecting { .. }) {
            retry.notify_one();
        }
        events.push(event);
    }
    events
}

#[tokio::test]
async fn a_dropped_stream_reconnects_from_just_before_the_last_line_without_repeats() {
    let (mut follower, requests) = follower(vec![
        Ok(vec![
            Ok(LogFrame::Line(item(10_000, "a"))),
            Ok(LogFrame::Line(item(10_500, "b"))),
            Err(LogDisconnect::Dropped),
        ]),
        Err(LogConnectError::Transient),
        Ok(vec![
            Ok(LogFrame::Line(item(10_500, "b"))),
            Ok(LogFrame::Gap { dropped: 3 }),
            Ok(LogFrame::Line(item(11_000, "c"))),
            Ok(LogFrame::End {
                reason: LogEndKind::ServiceRemoved,
                code: "service_removed".to_owned(),
                message: "The service was removed.".to_owned(),
            }),
        ]),
    ]);
    let events = drain(&mut follower).await;
    let line = |seq: u64, ts: i64, text: &str| {
        LogEvent::Line(LogLineItem {
            seq,
            ..item(ts, text)
        })
    };
    assert_eq!(
        events,
        vec![
            line(1, 10_000, "a"),
            line(2, 10_500, "b"),
            LogEvent::Reconnecting {
                attempt: 1,
                next_retry_ms: 1_000
            },
            LogEvent::Reconnecting {
                attempt: 2,
                next_retry_ms: 2_000
            },
            LogEvent::Reconnected,
            LogEvent::Gap { dropped: 3 },
            line(3, 11_000, "c"),
            LogEvent::Ended {
                reason: LogEndKind::ServiceRemoved,
                code: "service_removed".to_owned(),
                message: "The service was removed.".to_owned(),
            },
        ]
    );
    let requests = requests.lock().unwrap();
    assert_eq!((requests[0].since_unix_ms, requests[0].tail), (None, 500));
    for reopen in &requests[1..] {
        assert_eq!((reopen.since_unix_ms, reopen.tail), (Some(9_500), 5_000));
    }
}

#[tokio::test]
async fn transient_ends_retry_expiry_reconnects_at_once_and_terminal_ends_stop() {
    let (mut follower, requests) = follower(vec![
        Ok(vec![Ok(LogFrame::End {
            reason: LogEndKind::Error,
            code: "manager_unavailable".to_owned(),
            message: String::new(),
        })]),
        Ok(vec![Err(LogDisconnect::AccessExpired)]),
        Ok(vec![Ok(LogFrame::End {
            reason: LogEndKind::Error,
            code: "not_found".to_owned(),
            message: "No service has this id.".to_owned(),
        })]),
    ]);
    let events = drain(&mut follower).await;
    assert_eq!(
        events,
        vec![
            LogEvent::Reconnecting {
                attempt: 1,
                next_retry_ms: 1_000
            },
            LogEvent::Reconnected,
            LogEvent::Ended {
                reason: LogEndKind::Error,
                code: "not_found".to_owned(),
                message: "No service has this id.".to_owned(),
            },
        ]
    );
    assert_eq!(
        requests.lock().unwrap().len(),
        3,
        "expiry reconnected without a backoff"
    );

    let (mut refused, _) = follower_with_terminal("limit_reached");
    assert_eq!(
        drain(&mut refused).await,
        vec![LogEvent::Ended {
            reason: LogEndKind::Error,
            code: "limit_reached".to_owned(),
            message: "The log stream could not be opened.".to_owned(),
        }]
    );
}

fn follower_with_terminal(
    code: &str,
) -> (
    LogFollower<ScriptedConnector>,
    Arc<Mutex<Vec<LogOpenRequest>>>,
) {
    follower(vec![Err(LogConnectError::Terminal(code.to_owned()))])
}

#[tokio::test]
async fn cancellation_stops_a_follower_waiting_out_its_backoff() {
    let (mut follower, _) = follower(vec![Err(LogConnectError::Transient)]);
    let cancellation = CancellationToken::new();
    assert!(matches!(
        follower.next(&cancellation).await,
        Some(LogEvent::Reconnecting { attempt: 1, .. })
    ));
    cancellation.cancel();
    assert_eq!(follower.next(&cancellation).await, None);
}

/// Answers the open it receives with one line echoing the open, then ends the stream.
struct EchoOpen;

#[async_trait::async_trait]
impl StreamApplication for EchoOpen {
    async fn handle(
        &self,
        frame: StreamFrame,
        outbound: &OutboundSender,
    ) -> Result<(), OutboundFrameError> {
        {
            let Ok(LogMessage::Open(LogsOpen {
                service_id, tail, ..
            })) = LogMessage::from_frame(&frame)
            else {
                return Ok(());
            };
            outbound.try_send(
                LogMessage::Line(LogLine {
                    seq: 1,
                    ts_unix_ms: 1_790_000_000_000,
                    stream: LogStreamKind::Stderr,
                    task_id: "task".to_owned(),
                    node_id: "node".to_owned(),
                    line: format!("{service_id} tail={tail:?}"),
                    truncated: false,
                })
                .into_frame(1),
            )?;
            outbound.try_send(
                LogMessage::End(LogEnd {
                    reason: LogEndReason::ServiceRemoved,
                    code: "service_removed".to_owned(),
                    message: String::new(),
                })
                .into_frame(2),
            )
        }
    }
}

async fn upgrade(
    State(server): State<StreamServer>,
    headers: HeaderMap,
    websocket: WebSocketUpgrade,
) -> Response {
    let expected = format!("Bearer {ACCESS}");
    if headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        != Some(&expected)
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match server.admit(&websocket) {
        Ok(admission) => admission.upgrade(websocket),
        Err(_) => StatusCode::TOO_MANY_REQUESTS.into_response(),
    }
}

#[tokio::test]
async fn connect_logs_upgrades_with_the_bearer_and_opens_the_service() {
    let server = StreamServer::new(StreamLimits::default(), Arc::new(EchoOpen));
    let router = Router::new()
        .route("/api/v1/ws/logs", get(upgrade))
        .with_state(server.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let shutdown = CancellationToken::new();
    let stop = shutdown.clone();
    let http = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(stop.cancelled_owned())
            .await
            .unwrap();
    });
    let client = NativeClient::new(NativeClientConfig::new(format!("http://{address}"))).unwrap();
    let cancellation = CancellationToken::new();

    let wrong = AccessCredential::parse(&ACCESS.replace('1', "3")).unwrap();
    assert_eq!(
        client
            .connect_logs(&wrong, &request(), &cancellation)
            .await
            .err(),
        Some(NativeClientError::Unauthorized)
    );

    let access = AccessCredential::parse(ACCESS).unwrap();
    let mut receiver = client
        .connect_logs(&access, &request(), &cancellation)
        .await
        .unwrap();
    let Ok(LogFrame::Line(line)) = receiver.next_frame(&cancellation).await else {
        panic!("a line");
    };
    assert_eq!(line.text, format!("{SERVICE} tail=Some(500)"));
    assert_eq!(line.stream, LogStream::Stderr);
    assert!(matches!(
        receiver.next_frame(&cancellation).await,
        Ok(LogFrame::End {
            reason: LogEndKind::ServiceRemoved,
            ..
        })
    ));
    drop(receiver);
    server.shutdown().await.unwrap();
    shutdown.cancel();
    http.await.unwrap();
}
