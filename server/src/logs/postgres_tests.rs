//! Log streaming end to end: a real listener, real PostgreSQL sessions, and a fake manager whose
//! log lines the test writes.

use std::{
    error::Error,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use chrono::Utc;
use kontainer_engine::{
    EngineError, EngineErrorKind, EngineOperation, LogSource, ServiceLogLine, ServiceLogRequest,
    ServiceLogStream, ServiceSummary, SwarmStatus,
};
use kontainer_stream_protocol::{LogEndReason, LogMessage, LogStreamKind, LogsOpen};
use kontainer_stream_transport::{NativeStreamClient, StreamClientError, StreamLimits};
use loco_rs::{
    app::{AppContext, Hooks},
    config::Config,
    environment::Environment,
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use tokio::{net::TcpListener, sync::mpsc};
use tokio_util::sync::CancellationToken;

use crate::{
    app::App,
    auth::actions::create_session,
    logs::{LogStreams, domain::MAX_LOG_CONNECTIONS_PER_SESSION},
    swarm::engine::{ManagerEngine, ManagerError, SharedManagerEngine},
    test_support::PostgresTestDatabase,
};

const SERVICE: &str = "eb6ymskrt4ij70dco3menddxl";
const NODE: &str = "tj42rq5nzwf8r8avruy1a45b6";
const TASK: &str = "x0xza695s4xgf5rqmbvkpsizf";

type Lines = mpsc::UnboundedSender<Result<ServiceLogLine, EngineError>>;

/// Serves one service's logs from a channel the test writes; the service can be removed and the
/// manager made silent.
#[derive(Default)]
struct FakeManager {
    silent: AtomicBool,
    removed: AtomicBool,
    opens: Mutex<Vec<(String, ServiceLogRequest)>>,
    writer: Mutex<Option<Lines>>,
}

impl FakeManager {
    fn writer(&self) -> Lines {
        self.writer
            .lock()
            .unwrap()
            .clone()
            .expect("a stream is open")
    }
}

#[async_trait]
impl ManagerEngine for FakeManager {
    async fn list_networks(
        &self,
        _: &CancellationToken,
    ) -> Result<Vec<kontainer_engine::NetworkSummary>, ManagerError> {
        Ok(Vec::new())
    }

    async fn inspect_network(
        &self,
        _: &str,
        _: &CancellationToken,
    ) -> Result<kontainer_engine::NetworkDetail, ManagerError> {
        Err(ManagerError::NotFound)
    }

    async fn create_network(
        &self,
        _: &kontainer_engine::NetworkCreate,
        _: &CancellationToken,
    ) -> Result<String, ManagerError> {
        Err(ManagerError::Rejected)
    }

    async fn remove_network(&self, _: &str, _: &CancellationToken) -> Result<(), ManagerError> {
        Err(ManagerError::Rejected)
    }

    async fn swarm_status(&self, _: &CancellationToken) -> Result<SwarmStatus, ManagerError> {
        if self.silent.load(Ordering::SeqCst) {
            // A paused manager: the call never answers inside the deadline.
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
        Ok(SwarmStatus {
            control_available: true,
            managers: 1,
            nodes: 3,
        })
    }

    async fn list_services(
        &self,
        _: &CancellationToken,
    ) -> Result<Vec<ServiceSummary>, ManagerError> {
        if self.silent.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
        Ok(if self.removed.load(Ordering::SeqCst) {
            Vec::new()
        } else {
            vec![ServiceSummary {
                id: SERVICE.to_owned(),
                name: "webapp_web".to_owned(),
                ..ServiceSummary::default()
            }]
        })
    }

    async fn service_logs(
        &self,
        service_id: &str,
        request: ServiceLogRequest,
        _: &CancellationToken,
    ) -> Result<ServiceLogStream, ManagerError> {
        if service_id != SERVICE {
            // As Bollard does: the call succeeds and Docker's 404 is the stream's first item.
            let not_found =
                EngineError::new(EngineErrorKind::NotFound, EngineOperation::ReadServiceLogs);
            return Ok(Box::pin(futures_util::stream::iter([Err(not_found)])));
        }
        self.opens
            .lock()
            .unwrap()
            .push((service_id.to_owned(), request));
        let (writer, reader) = mpsc::unbounded_channel();
        *self.writer.lock().unwrap() = Some(writer);
        Ok(Box::pin(futures_util::stream::unfold(
            reader,
            |mut reader| async move { reader.recv().await.map(|item| (item, reader)) },
        )))
    }
}

fn line(text: &str, at: i64) -> ServiceLogLine {
    ServiceLogLine {
        source: LogSource::Stderr,
        timestamp_unix_ms: at,
        node_id: NODE.to_owned(),
        task_id: TASK.to_owned(),
        text: text.to_owned(),
    }
}

struct Fixture {
    endpoint: String,
    context: AppContext,
    manager: Arc<FakeManager>,
    access: String,
    http_shutdown: CancellationToken,
    http_task: tokio::task::JoinHandle<()>,
}

impl Fixture {
    async fn start(database: &sea_orm::DatabaseConnection) -> Result<Self, Box<dyn Error>> {
        let config = Config::from_folder(&Environment::Test, Path::new("config"))?;
        let context = App::after_context(
            AppContext::builder(Environment::Test, database.clone(), config).build(),
        )
        .await?;
        let manager = Arc::new(FakeManager::default());
        context.shared_store.insert(SharedManagerEngine(
            manager.clone() as Arc<dyn ManagerEngine>
        ));
        let mut streams = context.shared_store.get::<LogStreams>().unwrap();
        streams.liveness = Duration::from_millis(200);
        context.shared_store.insert(streams);
        let access = Self::session(database).await?;
        let router =
            App::routes(&context).to_router::<App>(context.clone(), axum::Router::new())?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let http_shutdown = CancellationToken::new();
        let shutdown = http_shutdown.clone();
        let http_task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
                .unwrap();
        });
        Ok(Self {
            endpoint: format!("ws://{address}/api/v1/ws/logs"),
            context,
            manager,
            access,
            http_shutdown,
            http_task,
        })
    }

    async fn session(database: &sea_orm::DatabaseConnection) -> Result<String, Box<dyn Error>> {
        let now = Utc::now();
        let principal = uuid::Uuid::now_v7();
        crate::auth::entities::principal::ActiveModel {
            id: Set(principal),
            email: Set(None),
            display_name: Set(None),
            kind: Set("human".to_owned()),
            status: Set("active".to_owned()),
            authorization_epoch: Set(0),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(database)
        .await?;
        Ok(create_session::execute(database, principal)
            .await?
            .access_plaintext)
    }

    async fn connect(&self, access: Option<&str>) -> Result<NativeStreamClient, StreamClientError> {
        NativeStreamClient::connect_with_bearer(
            &self.endpoint,
            access,
            StreamLimits::default(),
            &CancellationToken::new(),
        )
        .await
    }

    async fn open(&self, tail: Option<u32>) -> NativeStreamClient {
        let mut client = self.connect(Some(&self.access)).await.unwrap();
        let open = LogMessage::Open(LogsOpen {
            service_id: SERVICE.to_owned(),
            since_unix_ms: Some(1_790_000_000_000),
            tail,
        });
        client
            .send_frame(&open.into_frame(), &CancellationToken::new())
            .await
            .unwrap();
        client
    }

    async fn finish(self) {
        let streams = self.context.shared_store.get::<LogStreams>().unwrap();
        streams.server.shutdown().await.unwrap();
        self.http_shutdown.cancel();
        self.http_task.await.unwrap();
    }
}

async fn next(client: &mut NativeStreamClient) -> Result<LogMessage, StreamClientError> {
    let frame = tokio::time::timeout(
        Duration::from_secs(5),
        client.receive_frame(&CancellationToken::new()),
    )
    .await
    .expect("a frame or close within 5 s")?;
    Ok(LogMessage::from_frame(&frame).expect("a log payload"))
}

async fn opened(fixture: &Fixture) {
    for _ in 0..200 {
        if fixture.manager.writer.lock().unwrap().is_some() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the stream never opened");
}

#[tokio::test]
async fn postgres_log_streams_authenticate_follow_truncate_and_end() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = Fixture::start(&database.connection).await?;

        for access in [None, Some("kta_not_a_session")] {
            assert_eq!(
                fixture.connect(access).await.err(),
                Some(StreamClientError::HandshakeRejected { status: 401 }),
                "refused before the upgrade"
            );
        }

        let mut client = fixture.open(Some(20)).await;
        opened(&fixture).await;
        assert_eq!(
            fixture.manager.opens.lock().unwrap()[0].1,
            ServiceLogRequest {
                since_unix_ms: Some(1_790_000_000_000),
                tail: 20,
                follow: true
            }
        );
        let writer = fixture.manager.writer();
        writer.send(Ok(line("GET / 200", 1_790_000_000_001)))?;
        writer.send(Ok(line(&"x".repeat(70 * 1024), 1_790_000_000_002)))?;
        let LogMessage::Line(first) = next(&mut client).await.unwrap() else {
            panic!("a line first");
        };
        assert_eq!(
            (
                first.ts_unix_ms,
                first.line.as_str(),
                first.stream,
                first.task_id.as_str(),
                first.node_id.as_str()
            ),
            (
                1_790_000_000_001,
                "GET / 200",
                LogStreamKind::Stderr,
                TASK,
                NODE
            )
        );
        assert!(!first.truncated);
        let LogMessage::Line(long) = next(&mut client).await.unwrap() else {
            panic!("the long line");
        };
        assert!(long.truncated);
        assert_eq!(long.line.len(), 60 * 1024);

        // The service goes away. Docker keeps the follow open, so the liveness check notices.
        fixture.manager.removed.store(true, Ordering::SeqCst);
        let LogMessage::End(end) = next(&mut client).await.unwrap() else {
            panic!("an end");
        };
        assert_eq!(
            (end.reason, end.code.as_str()),
            (LogEndReason::ServiceRemoved, "service_removed")
        );
        assert_eq!(
            next(&mut client).await.err(),
            Some(StreamClientError::Closed {
                code: 1000,
                reason: "stream complete".to_owned()
            })
        );

        let mut unknown = fixture.connect(Some(&fixture.access)).await.unwrap();
        let open = LogMessage::Open(LogsOpen {
            service_id: "zzzzzzzzzzzzzzzzzzzzzzzzz".to_owned(),
            since_unix_ms: None,
            tail: None,
        });
        unknown
            .send_frame(&open.into_frame(), &CancellationToken::new())
            .await
            .unwrap();
        // Docker's 404 arrives as a stream item, and the service isn't listed: a terminal end,
        // not an outage the desktop would retry every second.
        let LogMessage::End(end) = next(&mut unknown).await.unwrap() else {
            panic!("an end");
        };
        assert_eq!(
            (end.reason, end.code.as_str()),
            (LogEndReason::ServiceRemoved, "service_removed")
        );

        // There is no close frame: closing the socket drops the session, which stops the follow.
        fixture.manager.removed.store(false, Ordering::SeqCst);
        *fixture.manager.writer.lock().unwrap() = None;
        let mut closing = fixture.open(None).await;
        opened(&fixture).await;
        let writer = fixture.manager.writer();
        closing.close().await;
        for _ in 0..200 {
            if writer.is_closed() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(writer.is_closed(), "the follow stopped with the socket");

        fixture.finish().await;
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_log_streams_limit_sessions_close_on_expiry_and_notice_a_silent_manager()
-> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = Fixture::start(&database.connection).await?;

        let mut held = Vec::new();
        for _ in 0..MAX_LOG_CONNECTIONS_PER_SESSION {
            held.push(fixture.connect(Some(&fixture.access)).await.unwrap());
        }
        assert_eq!(
            fixture.connect(Some(&fixture.access)).await.err(),
            Some(StreamClientError::HandshakeRejected { status: 429 }),
            "the ninth connection of one session"
        );
        let other = Fixture::session(&database.connection).await?;
        let mut elsewhere = fixture.connect(Some(&other)).await.unwrap();
        elsewhere.close().await;
        for mut client in held {
            client.close().await;
        }

        // A stream stops when the manager stops answering its liveness checks.
        let mut client = fixture.open(None).await;
        opened(&fixture).await;
        fixture.manager.silent.store(true, Ordering::SeqCst);
        let LogMessage::End(end) = next(&mut client).await.unwrap() else {
            panic!("an end");
        };
        assert_eq!(
            (end.reason, end.code.as_str()),
            (LogEndReason::Error, "manager_unavailable")
        );
        assert_eq!(
            fixture.connect(Some(&fixture.access)).await.err(),
            Some(StreamClientError::HandshakeRejected { status: 503 }),
            "a silent manager refuses new upgrades, so reconnects back off"
        );
        fixture.manager.silent.store(false, Ordering::SeqCst);

        // An access credential that expires mid-stream closes it with 4401.
        let expiring = Fixture::session(&database.connection).await?;
        let family = crate::auth::entities::session_family::Entity::find()
            .all(&database.connection)
            .await?
            .into_iter()
            .max_by_key(|family| family.created_at)
            .unwrap();
        let mut family: crate::auth::entities::session_family::ActiveModel = family.into();
        family.access_expires_at = Set(Utc::now() + chrono::Duration::milliseconds(1500));
        family.update(&database.connection).await?;
        let mut client = fixture.connect(Some(&expiring)).await.unwrap();
        let open = LogMessage::Open(LogsOpen {
            service_id: SERVICE.to_owned(),
            since_unix_ms: None,
            tail: None,
        });
        client
            .send_frame(&open.into_frame(), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(
            next(&mut client).await.err(),
            Some(StreamClientError::Closed {
                code: 4401,
                reason: "access expired".to_owned()
            })
        );

        fixture.finish().await;
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}
