//! Log streams: the authenticated connect, the IPC event contract, line batching, and the
//! registry's cancellation, with shells, on reload and window close.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use kontainer_native_client::{
    ExecConnection, LogConnectError, LogConnectFuture, LogConnector, LogDisconnect, LogEndKind,
    LogFrame, LogLineItem, LogOpenRequest, LogReceiver, LogStream, NativeClientError, ServiceId,
};
use tokio_util::sync::CancellationToken;

use super::fakes::{
    future_ms, issued, Harness, RecordedExecInput, ScriptedExecEvents, ScriptedLogReceiver,
};
use crate::session::exec_sessions::{ExecEventSink, ExecSessions, ExecStreamEvent};
use crate::session::log_streams::{LogEventSink, LogStreamEvent, LogStreams};
use crate::session::manager::{log_connect_error, InventoryError};

const FIXTURE: &str = include_str!("../../../../tests/fixtures/logs-ipc.json");
const LIB_SOURCE: &str = include_str!("../../lib.rs");
const LOGOUT_SOURCE: &str = include_str!("../../actions/session/session_logout.rs");
const SERVICE: &str = "eb6ymskrt4ij70dco3menddxl";

fn request() -> LogOpenRequest {
    LogOpenRequest {
        service_id: ServiceId::parse(SERVICE).unwrap(),
        since_unix_ms: Some(1_790_000_000_000),
        tail: 500,
    }
}

fn line(ts: i64, text: &str) -> LogLineItem {
    LogLineItem {
        seq: 0,
        ts_unix_ms: ts,
        stream: LogStream::Stderr,
        task_id: "x0xza695s4xgf5rqmbvkpsizf".to_owned(),
        node_id: "tj42rq5nzwf8r8avruy1a45b6".to_owned(),
        text: text.to_owned(),
        truncated: false,
    }
}

#[tokio::test]
async fn connects_with_the_session_credential_and_refreshes_once_on_401() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    {
        let mut script = harness.controllers.script();
        script
            .log_connections
            .push_back(Err(NativeClientError::Unauthorized));
        script
            .refresh
            .push_back(Ok(issued(2, future_ms(), future_ms())));
        script
            .log_connections
            .push_back(Ok(vec![Ok(LogFrame::Line(line(1, "hello")))]));
    }
    let mut receiver = harness
        .manager
        .connect_logs(view.profile_id, request())
        .await
        .unwrap();
    assert_eq!(
        receiver.next_frame(&CancellationToken::new()).await,
        Ok(LogFrame::Line(line(1, "hello")))
    );
    assert_eq!(harness.count("logs:"), 2);
    assert_eq!(harness.count("refresh:"), 1);
    assert!(harness
        .entries()
        .iter()
        .any(|entry| entry.starts_with("logs:")
            && entry.ends_with(&format!(":{SERVICE}:Some(1790000000000):500"))));

    harness
        .controllers
        .script()
        .log_connections
        .push_back(Err(NativeClientError::LimitReached));
    assert_eq!(
        harness
            .manager
            .connect_logs(view.profile_id, request())
            .await
            .err(),
        Some(InventoryError::LimitReached)
    );
}

#[test]
fn only_an_unreachable_controller_or_manager_is_worth_retrying() {
    for transient in [
        InventoryError::ControllerUnavailable,
        InventoryError::QuorumUnavailable,
        InventoryError::AgentTimeout,
    ] {
        assert_eq!(log_connect_error(transient), LogConnectError::Transient);
    }
    for (terminal, code) in [
        (InventoryError::ReloginRequired, "relogin_required"),
        (InventoryError::LimitReached, "limit_reached"),
        (InventoryError::NotAManager, "not_a_manager"),
        (InventoryError::InvalidRequest, "invalid_request"),
    ] {
        assert_eq!(
            log_connect_error(terminal),
            LogConnectError::Terminal(code.to_owned())
        );
    }
}

#[test]
fn stream_events_match_the_shared_fixture_and_commands_are_registered() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let events: Vec<serde_json::Value> = [
        LogStreamEvent::Lines {
            lines: vec![LogLineItem {
                seq: 1,
                ..line(1_790_000_000_123, "GET / 200")
            }
            .into()],
        },
        LogStreamEvent::Gap { dropped: 42 },
        LogStreamEvent::Reconnecting {
            attempt: 3,
            next_retry_ms: 4_000,
        },
        LogStreamEvent::Reconnected,
        LogStreamEvent::Ended {
            reason: "service_removed",
            code: "service_removed".to_owned(),
            message: "The service was removed.".to_owned(),
        },
    ]
    .into_iter()
    .map(|event| serde_json::to_value(event).unwrap())
    .collect();
    assert_eq!(serde_json::Value::from(events), fixture["events"]);
    for command in fixture["commands"].as_array().unwrap() {
        let domain = command[0].as_str().unwrap();
        let name = command[1].as_str().unwrap();
        assert!(
            LIB_SOURCE.contains(&format!("actions::{domain}::{name}::{name}")),
            "{name} is registered"
        );
    }
    assert!(
        LIB_SOURCE.contains("cancel_all()"),
        "window close cancels streams"
    );
    assert!(
        LOGOUT_SOURCE.contains("streams.cancel_all()"),
        "logout cancels streams"
    );
}

/// Collects what React would receive.
#[derive(Clone, Default)]
struct Collected(Arc<Mutex<Vec<LogStreamEvent>>>);

impl LogEventSink for Collected {
    fn deliver(&self, event: LogStreamEvent) -> bool {
        self.0.lock().unwrap().push(event);
        true
    }
}

impl Collected {
    fn events(&self) -> Vec<LogStreamEvent> {
        self.0.lock().unwrap().clone()
    }
}

/// Hands out one scripted connection per connect.
struct Scripted(Vec<Vec<Result<LogFrame, LogDisconnect>>>);

impl LogConnector for Scripted {
    fn connect<'a>(
        &'a mut self,
        _request: LogOpenRequest,
        _cancellation: &'a CancellationToken,
    ) -> LogConnectFuture<'a> {
        let next = if self.0.is_empty() {
            Err(LogConnectError::Terminal("script_ended".to_owned()))
        } else {
            Ok(Box::new(ScriptedLogReceiver(self.0.remove(0).into())) as Box<dyn LogReceiver>)
        };
        Box::pin(async move { next })
    }
}

async fn settle(streams: &LogStreams) {
    for _ in 0..200 {
        if streams.len() == 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("the stream task never finished");
}

#[tokio::test]
async fn lines_cross_in_batches_and_other_events_flush_them_first() {
    let streams = LogStreams::default();
    let sink = Collected::default();
    let frames = vec![
        Ok(LogFrame::Line(line(1, "a"))),
        Ok(LogFrame::Line(line(2, "b"))),
        Ok(LogFrame::Line(line(3, "c"))),
        Ok(LogFrame::Gap { dropped: 7 }),
        Ok(LogFrame::Line(line(4, "d"))),
        Ok(LogFrame::End {
            reason: LogEndKind::ServiceRemoved,
            code: "service_removed".to_owned(),
            message: "The service was removed.".to_owned(),
        }),
    ];
    streams.open(Scripted(vec![frames]), request(), sink.clone());
    settle(&streams).await;
    let texts = |event: &LogStreamEvent| match event {
        LogStreamEvent::Lines { lines } => serde_json::to_value(lines)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|line| line["text"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    let events = sink.events();
    assert_eq!(events.len(), 4, "{events:?}");
    assert_eq!(
        texts(&events[0]),
        vec!["a", "b", "c"],
        "one batch, not three events"
    );
    assert_eq!(events[1], LogStreamEvent::Gap { dropped: 7 });
    assert_eq!(texts(&events[2]), vec!["d"]);
    assert!(matches!(
        events[3],
        LogStreamEvent::Ended {
            reason: "service_removed",
            ..
        }
    ));
}

/// `lib.rs` from `start` up to the next `end`.
fn lib_section(start: &str, end: &str) -> &'static str {
    let from = LIB_SOURCE
        .find(start)
        .unwrap_or_else(|| panic!("lib.rs has {start}"));
    let section = &LIB_SOURCE[from..];
    &section[..section.find(end).unwrap()]
}

#[tokio::test]
async fn a_reload_or_window_close_ends_every_stream_and_shell() {
    let page_load = lib_section(".on_page_load(", "})");
    assert!(
        page_load.contains("PageLoadEvent::Started")
            && page_load.contains("end_page_streams(webview)")
    );
    let window = lib_section(".on_window_event(", "})");
    assert!(
        window.contains("WindowEvent::Destroyed") && window.contains("end_page_streams(window)")
    );
    // The closing brace at column 0; "\n}" matches LF and CRLF checkouts (Windows CI).
    let ends = lib_section("fn end_page_streams", "\n}");
    assert!(ends.contains("LogStreams>()") && ends.contains("streams.cancel_all()"));
    assert!(ends.contains("ExecSessions>()") && ends.contains("shells.cancel_all()"));

    // What the hook does, against both registries with something open in each.
    let streams = LogStreams::default();
    let shells = ExecSessions::default();
    streams.open(
        Scripted(vec![vec![Ok(LogFrame::Line(line(1, "a")))]]),
        request(),
        Collected::default(),
    );
    shells.open(
        ExecConnection {
            input: Box::new(RecordedExecInput(Arc::default())),
            events: Box::new(ScriptedExecEvents(VecDeque::new())),
        },
        DiscardedExec,
    );
    assert_eq!((streams.len(), shells.len()), (1, 1));
    streams.cancel_all();
    shells.cancel_all();
    settle(&streams).await;
    assert_eq!(shells.len(), 0);
}

struct DiscardedExec;

impl ExecEventSink for DiscardedExec {
    fn deliver(&self, _: ExecStreamEvent) -> bool {
        true
    }
}

#[tokio::test]
async fn closing_and_cancel_all_stop_open_streams() {
    let streams = LogStreams::default();
    let quiet = || Scripted(vec![vec![Ok(LogFrame::Line(line(1, "a")))]]);
    let first = streams.open(quiet(), request(), Collected::default());
    let second = streams.open(quiet(), request(), Collected::default());
    streams.open(quiet(), request(), Collected::default());
    assert_eq!(streams.len(), 3);
    assert!(streams.retry_now(first));
    assert!(streams.close(first));
    assert!(!streams.close(first), "already closed");
    streams.cancel_all();
    assert!(!streams.retry_now(second));
    settle(&streams).await;
}
