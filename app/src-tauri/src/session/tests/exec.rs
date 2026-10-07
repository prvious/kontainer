//! Exec sessions: the authenticated connect, the IPC event contract, the relay, and the
//! registry's cancellation.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use kontainer_native_client::{
    ExecClosedReason, ExecEvent, ExecInputFrame, ExecOpenRequest, NativeClientError, NodeId,
};

use super::fakes::{future_ms, issued, Harness};
use crate::session::exec_sessions::{ExecEventSink, ExecSessions, ExecStreamEvent};
use crate::session::manager::InventoryError;

const FIXTURE: &str = include_str!("../../../../tests/fixtures/exec-ipc.json");
const LIB_SOURCE: &str = include_str!("../../lib.rs");
const LOGOUT_SOURCE: &str = include_str!("../../actions/session/session_logout.rs");

fn request() -> ExecOpenRequest {
    ExecOpenRequest {
        node_id: NodeId::parse("manager").unwrap(),
        container_id: "c".repeat(64),
        shell: None,
        cols: 120,
        rows: 40,
    }
}

#[derive(Clone, Default)]
struct Collected(Arc<Mutex<Vec<ExecStreamEvent>>>);

impl ExecEventSink for Collected {
    fn deliver(&self, event: ExecStreamEvent) -> bool {
        self.0.lock().unwrap().push(event);
        true
    }
}

impl Collected {
    fn events(&self) -> Vec<ExecStreamEvent> {
        self.0.lock().unwrap().clone()
    }
}

async fn settle(sessions: &ExecSessions) {
    for _ in 0..200 {
        if sessions.len() == 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("the session never ended");
}

#[test]
fn stream_events_match_the_shared_fixture_and_commands_are_registered() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let events: Vec<serde_json::Value> = [
        ExecEvent::Started {
            exec_id: "e".repeat(64),
            shell: "/bin/sh".to_owned(),
        },
        ExecEvent::Output(b"ok\r\n".to_vec()),
        ExecEvent::Exit {
            code: Some(0),
            reason: "exited".to_owned(),
        },
        ExecEvent::Exit {
            code: None,
            reason: "idle_timeout".to_owned(),
        },
        ExecEvent::Error {
            code: "shell_not_found".to_owned(),
            message: "No shell at /bin/zsh in this container.".to_owned(),
        },
        ExecEvent::Closed {
            reason: ExecClosedReason::ConnectionLost,
        },
    ]
    .into_iter()
    .map(|event| serde_json::to_value(ExecStreamEvent::from(event)).unwrap())
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
        LIB_SOURCE.contains("ExecSessions>()"),
        "window close ends shells"
    );
    assert!(
        LOGOUT_SOURCE.contains("shells.cancel_all()"),
        "logout ends shells"
    );
}

#[tokio::test]
async fn connects_with_the_session_credential_and_refreshes_once_on_401() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    {
        let mut script = harness.controllers.script();
        script
            .exec_connections
            .push_back(Err(NativeClientError::Unauthorized));
        script
            .refresh
            .push_back(Ok(issued(2, future_ms(), future_ms())));
        script.exec_connections.push_back(Ok(Vec::new()));
        script
            .exec_connections
            .push_back(Err(NativeClientError::LimitReached));
    }
    assert!(harness
        .manager
        .connect_exec(view.profile_id, request())
        .await
        .is_ok());
    assert_eq!(harness.count("exec:"), 2);
    assert_eq!(harness.count("refresh:"), 1);
    assert!(harness.entries().iter().any(
        |entry| entry.starts_with("exec:") && entry.ends_with(":manager:cccccccc:None:120x40")
    ));
    assert_eq!(
        harness
            .manager
            .connect_exec(view.profile_id, request())
            .await
            .err(),
        Some(InventoryError::LimitReached)
    );
}

#[tokio::test]
async fn a_session_relays_events_until_it_closes_and_takes_input() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    let inputs = harness.controllers.script().exec_inputs.clone();
    harness
        .controllers
        .script()
        .exec_connections
        .push_back(Ok(vec![
            ExecEvent::Started {
                exec_id: "e".repeat(64),
                shell: "/bin/sh".to_owned(),
            },
            ExecEvent::Output(b"$ ".to_vec()),
        ]));
    let connection = harness
        .manager
        .connect_exec(view.profile_id, request())
        .await
        .unwrap();
    let sessions = ExecSessions::default();
    let sink = Collected::default();
    let id = sessions.open(connection, sink.clone());
    assert!(
        sessions
            .send(id, ExecInputFrame::Input(b"ls\r".to_vec()))
            .await
    );
    assert!(
        sessions
            .send(id, ExecInputFrame::Resize { cols: 90, rows: 20 })
            .await
    );
    assert!(sessions.close(id).await);
    assert_eq!(
        inputs.lock().unwrap().clone(),
        vec![
            ExecInputFrame::Input(b"ls\r".to_vec()),
            ExecInputFrame::Resize { cols: 90, rows: 20 },
            ExecInputFrame::Close
        ]
    );
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(
        sink.events(),
        vec![
            ExecStreamEvent::Started {
                shell: "/bin/sh".to_owned()
            },
            ExecStreamEvent::Output {
                data: "JCA=".to_owned()
            }
        ]
    );

    // Logout ends every open shell; a closed session takes nothing more.
    sessions.cancel_all();
    settle(&sessions).await;
    assert!(
        !sessions
            .send(id, ExecInputFrame::Input(b"x".to_vec()))
            .await
    );
}

#[tokio::test]
async fn a_close_event_is_the_last_one_and_ends_the_session() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    harness
        .controllers
        .script()
        .exec_connections
        .push_back(Ok(vec![
            ExecEvent::Exit {
                code: Some(0),
                reason: "exited".to_owned(),
            },
            ExecEvent::Closed {
                reason: ExecClosedReason::Closed,
            },
        ]));
    let connection = harness
        .manager
        .connect_exec(view.profile_id, request())
        .await
        .unwrap();
    let sessions = ExecSessions::default();
    let sink = Collected::default();
    sessions.open(connection, sink.clone());
    settle(&sessions).await;
    assert_eq!(
        sink.events(),
        vec![
            ExecStreamEvent::Exit {
                code: Some(0),
                reason: "exited".to_owned()
            },
            ExecStreamEvent::Closed { reason: "closed" }
        ]
    );
}
