//! Exec sessions: local validation, event mapping, and close reasons against a loopback stream
//! server.

use std::sync::{Arc, Mutex};

use axum::{
    Router,
    extract::{State, ws::WebSocketUpgrade},
    response::Response,
    routing::get,
};
use kontainer_stream_protocol::{ExecExit, ExecMessage, ExecStarted, StreamFrame};
use kontainer_stream_transport::{
    OutboundFrameError, OutboundSender, StreamApplication, StreamClose, StreamLimits, StreamServer,
};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::exec::{ExecClosedReason, ExecEvent, ExecInputFrame, ExecOpenRequest};
use crate::{AccessCredential, NativeClient, NativeClientConfig, NativeClientError, NodeId};

const ACCESS: &str = "kta_11111111111111111111111111111111.2222222222222222222222222222222222222222222222222222222222222222";

fn request() -> ExecOpenRequest {
    ExecOpenRequest {
        node_id: NodeId::parse("manager").unwrap(),
        container_id: "c".repeat(64),
        shell: None,
        cols: 80,
        rows: 24,
    }
}

/// A shell that echoes input, records resizes, and exits 0 on close, or closes 4401 when asked
/// to with `expire`.
#[derive(Default)]
struct EchoShell {
    resizes: Mutex<Vec<(u32, u32)>>,
}

#[async_trait::async_trait]
impl StreamApplication for EchoShell {
    async fn handle(
        &self,
        frame: StreamFrame,
        outbound: &OutboundSender,
    ) -> Result<(), OutboundFrameError> {
        match ExecMessage::from_frame(&frame) {
            Ok(ExecMessage::Open(open)) => outbound.try_send(
                ExecMessage::Started(ExecStarted {
                    exec_id: "e".repeat(64),
                    shell: open.shell.unwrap_or_else(|| "/bin/sh".to_owned()),
                })
                .into_frame(),
            ),
            Ok(ExecMessage::Input(bytes)) if bytes == b"expire" => {
                outbound.close(StreamClose::new(4401, "access expired"));
                Ok(())
            }
            Ok(ExecMessage::Input(bytes)) => {
                outbound.try_send(ExecMessage::Output(bytes).into_frame())
            }
            Ok(ExecMessage::Resize(resize)) => {
                self.resizes
                    .lock()
                    .unwrap()
                    .push((resize.cols, resize.rows));
                Ok(())
            }
            Ok(ExecMessage::Close) => {
                outbound.try_send(
                    ExecMessage::Exit(ExecExit {
                        code: Some(0),
                        reason: "exited".to_owned(),
                    })
                    .into_frame(),
                )?;
                outbound.close(StreamClose::NORMAL);
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

async fn upgrade(State(server): State<StreamServer>, websocket: WebSocketUpgrade) -> Response {
    server.admit(&websocket).unwrap().upgrade(websocket)
}

async fn serve(shell: Arc<EchoShell>) -> (NativeClient, StreamServer, CancellationToken) {
    let server = StreamServer::new(StreamLimits::default(), shell);
    let router = Router::new()
        .route("/api/v1/ws/exec", get(upgrade))
        .with_state(server.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let shutdown = CancellationToken::new();
    let stop = shutdown.clone();
    tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(stop.cancelled_owned())
            .await
            .unwrap();
    });
    let client = NativeClient::new(NativeClientConfig::new(format!("http://{address}"))).unwrap();
    (client, server, shutdown)
}

#[tokio::test]
async fn requests_are_checked_before_any_connection() {
    let client = NativeClient::new(NativeClientConfig::new("http://127.0.0.1:1")).unwrap();
    let access = AccessCredential::parse(ACCESS).unwrap();
    let cancellation = CancellationToken::new();
    for bad in [
        ExecOpenRequest {
            container_id: "web".to_owned(),
            ..request()
        },
        ExecOpenRequest {
            shell: Some(String::new()),
            ..request()
        },
        ExecOpenRequest {
            cols: 0,
            ..request()
        },
        ExecOpenRequest {
            rows: 1_001,
            ..request()
        },
    ] {
        assert_eq!(
            client
                .connect_exec(&access, &bad, &cancellation)
                .await
                .err(),
            Some(NativeClientError::InvalidRequest),
            "{bad:?}"
        );
    }
}

#[tokio::test]
async fn a_session_starts_echoes_resizes_and_exits_then_reports_the_close() {
    let shell = Arc::new(EchoShell::default());
    let (client, server, shutdown) = serve(shell.clone()).await;
    let access = AccessCredential::parse(ACCESS).unwrap();
    let cancellation = CancellationToken::new();
    let mut session = client
        .connect_exec(&access, &request(), &cancellation)
        .await
        .unwrap();
    assert_eq!(
        session.events.next_event(&cancellation).await,
        Some(ExecEvent::Started {
            exec_id: "e".repeat(64),
            shell: "/bin/sh".to_owned()
        })
    );
    session
        .input
        .send(ExecInputFrame::Input(b"ls\r".to_vec()), &cancellation)
        .await
        .unwrap();
    assert_eq!(
        session.events.next_event(&cancellation).await,
        Some(ExecEvent::Output(b"ls\r".to_vec()))
    );
    session
        .input
        .send(
            ExecInputFrame::Resize {
                cols: 100,
                rows: 30,
            },
            &cancellation,
        )
        .await
        .unwrap();
    assert_eq!(
        session
            .input
            .send(ExecInputFrame::Resize { cols: 0, rows: 30 }, &cancellation)
            .await,
        Err(NativeClientError::InvalidRequest)
    );
    session
        .input
        .send(ExecInputFrame::Close, &cancellation)
        .await
        .unwrap();
    assert_eq!(
        session.events.next_event(&cancellation).await,
        Some(ExecEvent::Exit {
            code: Some(0),
            reason: "exited".to_owned()
        })
    );
    assert_eq!(
        session.events.next_event(&cancellation).await,
        Some(ExecEvent::Closed {
            reason: ExecClosedReason::Closed
        })
    );
    assert_eq!(session.events.next_event(&cancellation).await, None);
    assert_eq!(shell.resizes.lock().unwrap().as_slice(), &[(100, 30)]);
    drop(session);
    server.shutdown().await.unwrap();
    shutdown.cancel();
}

#[tokio::test]
async fn an_expired_session_closes_as_session_expired() {
    let (client, server, shutdown) = serve(Arc::new(EchoShell::default())).await;
    let access = AccessCredential::parse(ACCESS).unwrap();
    let cancellation = CancellationToken::new();
    let mut session = client
        .connect_exec(&access, &request(), &cancellation)
        .await
        .unwrap();
    assert!(matches!(
        session.events.next_event(&cancellation).await,
        Some(ExecEvent::Started { .. })
    ));
    session
        .input
        .send(ExecInputFrame::Input(b"expire".to_vec()), &cancellation)
        .await
        .unwrap();
    assert_eq!(
        session.events.next_event(&cancellation).await,
        Some(ExecEvent::Closed {
            reason: ExecClosedReason::SessionExpired
        })
    );
    drop(session);
    server.shutdown().await.unwrap();
    shutdown.cancel();
}
