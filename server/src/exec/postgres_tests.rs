//! Exec end to end: a real listener, real PostgreSQL membership and sessions, and a fake agent
//! whose events the test writes and whose input the test reads.

use std::{
    error::Error,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::Utc;
use kontainer_stream_protocol::{ExecMessage, ExecOpen, ExecResize};
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
    exec::{ExecStreams, domain::MAX_EXEC_SESSIONS_PER_SESSION},
    nodes::{
        actions::record_check_in,
        agents::{AgentDirectory, NodeEndpoint, SharedAgentDirectory},
        postgres_tests::check_in,
    },
    rpc::{
        AgentClient, AgentClientError, AgentContainerPage, AgentDescription, AgentExecEvent,
        AgentExecInput, AgentExecOpen, AgentExecStream,
    },
    test_support::PostgresTestDatabase,
};

const CONTAINER: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

/// Records what the controller sends the agent and plays the events the test writes.
#[derive(Debug, Default)]
struct FakeAgent {
    opens: Mutex<Vec<AgentExecOpen>>,
    received: Arc<Mutex<Vec<AgentExecInput>>>,
    events: Mutex<Option<mpsc::UnboundedSender<AgentExecEvent>>>,
}

impl FakeAgent {
    fn events(&self) -> mpsc::UnboundedSender<AgentExecEvent> {
        self.events
            .lock()
            .unwrap()
            .clone()
            .expect("an exec is open")
    }

    fn received(&self) -> Vec<AgentExecInput> {
        self.received.lock().unwrap().clone()
    }
}

#[derive(Debug)]
struct SharedAgent(Arc<FakeAgent>);

#[async_trait::async_trait]
impl AgentClient for SharedAgent {
    async fn describe(&self, _: &CancellationToken) -> Result<AgentDescription, AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    async fn list_containers(
        &self,
        _: u32,
        _: String,
        _: &CancellationToken,
    ) -> Result<AgentContainerPage, AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    fn node_id(&self) -> &str {
        "manager"
    }

    async fn exec(
        &self,
        open: AgentExecOpen,
        mut input: mpsc::Receiver<AgentExecInput>,
        _: &CancellationToken,
    ) -> Result<AgentExecStream, AgentClientError> {
        self.0.opens.lock().unwrap().push(open);
        let received = self.0.received.clone();
        tokio::spawn(async move {
            while let Some(message) = input.recv().await {
                received.lock().unwrap().push(message);
            }
        });
        let (sender, receiver) = mpsc::unbounded_channel();
        *self.0.events.lock().unwrap() = Some(sender);
        Ok(Box::pin(
            tokio_stream::wrappers::UnboundedReceiverStream::new(receiver),
        ))
    }
}

#[derive(Debug)]
struct FakeDirectory(Arc<FakeAgent>);

impl AgentDirectory for FakeDirectory {
    fn client(&self, _: &NodeEndpoint) -> Result<Arc<dyn AgentClient>, AgentClientError> {
        Ok(Arc::new(SharedAgent(self.0.clone())))
    }
}

struct Fixture {
    endpoint: String,
    context: AppContext,
    agent: Arc<FakeAgent>,
    access: String,
    http_shutdown: CancellationToken,
    http_task: tokio::task::JoinHandle<()>,
}

impl Fixture {
    async fn start(
        database: &sea_orm::DatabaseConnection,
        idle: Duration,
        stall: Duration,
    ) -> Result<Self, Box<dyn Error>> {
        let config = Config::from_folder(&Environment::Test, Path::new("config"))?;
        let context = App::after_context(
            AppContext::builder(Environment::Test, database.clone(), config).build(),
        )
        .await?;
        let agent = Arc::new(FakeAgent::default());
        context
            .shared_store
            .insert(SharedAgentDirectory(Arc::new(FakeDirectory(agent.clone()))));
        let mut streams = context.shared_store.get::<ExecStreams>().unwrap();
        streams.timing.idle = idle;
        streams.timing.stall = stall;
        context.shared_store.insert(streams);
        record_check_in::execute(database, "development", check_in("manager", None)).await?;
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
            endpoint: format!("ws://{address}/api/v1/ws/exec"),
            context,
            agent,
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

    async fn connect(&self, access: &str) -> Result<NativeStreamClient, StreamClientError> {
        NativeStreamClient::connect_with_bearer(
            &self.endpoint,
            Some(access),
            StreamLimits::default(),
            &CancellationToken::new(),
        )
        .await
    }

    async fn open(&self, access: &str, node: &str) -> NativeStreamClient {
        // The next exec the agent sees is this one's.
        *self.agent.events.lock().unwrap() = None;
        let mut client = self.connect(access).await.unwrap();
        let open = ExecMessage::Open(ExecOpen {
            node_id: node.to_owned(),
            container_id: CONTAINER.to_owned(),
            shell: None,
            cols: 120,
            rows: 40,
        });
        client
            .send_frame(&open.into_frame(1), &CancellationToken::new())
            .await
            .unwrap();
        client
    }

    async fn finish(self) {
        let streams = self.context.shared_store.get::<ExecStreams>().unwrap();
        streams.server.shutdown().await.unwrap();
        self.http_shutdown.cancel();
        self.http_task.await.unwrap();
    }
}

async fn next(client: &mut NativeStreamClient) -> Result<ExecMessage, StreamClientError> {
    let frame = tokio::time::timeout(
        Duration::from_secs(5),
        client.receive_frame(&CancellationToken::new()),
    )
    .await
    .expect("a frame or close within 5 s")?;
    Ok(ExecMessage::from_frame(&frame).expect("an exec payload"))
}

async fn opened(agent: &FakeAgent) {
    for _ in 0..200 {
        if agent.events.lock().unwrap().is_some() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the agent exec never opened");
}

async fn wait_for(agent: &FakeAgent, count: usize) {
    for _ in 0..200 {
        if agent.received().len() >= count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("expected {count} inputs, got {:?}", agent.received());
}

#[tokio::test]
async fn postgres_exec_relays_a_shell_both_ways_and_reports_its_exit() -> Result<(), Box<dyn Error>>
{
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = Fixture::start(
            &database.connection,
            Duration::from_secs(60),
            Duration::from_secs(10),
        )
        .await?;
        assert_eq!(
            fixture.connect("kta_not_a_session").await.err(),
            Some(StreamClientError::HandshakeRejected { status: 401 })
        );

        let mut client = fixture.open(&fixture.access, "manager").await;
        opened(&fixture.agent).await;
        assert_eq!(
            fixture.agent.opens.lock().unwrap()[0],
            AgentExecOpen {
                container_id: CONTAINER.to_owned(),
                shell: None,
                cols: 120,
                rows: 40
            }
        );
        let events = fixture.agent.events();
        events.send(AgentExecEvent::Started {
            exec_id: "e".repeat(64),
            shell: "/bin/sh".to_owned(),
        })?;
        let ExecMessage::Started(started) = next(&mut client).await.unwrap() else {
            panic!("started");
        };
        assert_eq!(started.shell, "/bin/sh");

        let cancellation = CancellationToken::new();
        client
            .send_frame(
                &ExecMessage::Input(b"echo ok\r".to_vec()).into_frame(2),
                &cancellation,
            )
            .await
            .unwrap();
        client
            .send_frame(
                &ExecMessage::Resize(ExecResize { cols: 90, rows: 20 }).into_frame(3),
                &cancellation,
            )
            .await
            .unwrap();
        wait_for(&fixture.agent, 2).await;
        assert_eq!(
            fixture.agent.received(),
            vec![
                AgentExecInput::Input(b"echo ok\r".to_vec()),
                AgentExecInput::Resize { cols: 90, rows: 20 }
            ]
        );

        // Output larger than one frame arrives in 32 KiB pieces, in order.
        let big: Vec<u8> = (0..70 * 1024).map(|index| (index % 251) as u8).collect();
        events.send(AgentExecEvent::Output(big.clone()))?;
        let mut received = Vec::new();
        let mut frames = 0;
        while received.len() < big.len() {
            let ExecMessage::Output(bytes) = next(&mut client).await.unwrap() else {
                panic!("output");
            };
            assert!(bytes.len() <= 32 * 1024);
            received.extend(bytes);
            frames += 1;
        }
        assert_eq!((received == big, frames), (true, 3));

        client
            .send_frame(&ExecMessage::Close.into_frame(4), &cancellation)
            .await
            .unwrap();
        wait_for(&fixture.agent, 3).await;
        assert_eq!(fixture.agent.received()[2], AgentExecInput::Close);
        events.send(AgentExecEvent::Exit { code: Some(0) })?;
        let ExecMessage::Exit(exit) = next(&mut client).await.unwrap() else {
            panic!("an exit");
        };
        assert_eq!((exit.code, exit.reason.as_str()), (Some(0), "exited"));
        assert_eq!(
            next(&mut client).await.err(),
            Some(StreamClientError::Closed {
                code: 1000,
                reason: "stream complete".to_owned()
            })
        );

        // The agent's engine failure reads as the node's; an unknown node never reaches an agent.
        let mut failing = fixture.open(&fixture.access, "manager").await;
        opened(&fixture.agent).await;
        fixture.agent.events().send(AgentExecEvent::Error {
            code: "engine_error".to_owned(),
            message: "Docker didn't answer on this node.".to_owned(),
        })?;
        let ExecMessage::Error(error) = next(&mut failing).await.unwrap() else {
            panic!("an error");
        };
        assert_eq!(error.code, "node_unavailable");
        let mut unknown = fixture.open(&fixture.access, "gone").await;
        let ExecMessage::Error(error) = next(&mut unknown).await.unwrap() else {
            panic!("an error");
        };
        assert_eq!(error.code, "node_unavailable");

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
async fn postgres_exec_limits_sessions_and_ends_idle_expired_and_stalled_ones()
-> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = Fixture::start(
            &database.connection,
            Duration::from_millis(400),
            Duration::from_millis(300),
        )
        .await?;

        let mut held = Vec::new();
        for _ in 0..MAX_EXEC_SESSIONS_PER_SESSION {
            held.push(fixture.connect(&fixture.access).await.unwrap());
        }
        assert_eq!(
            fixture.connect(&fixture.access).await.err(),
            Some(StreamClientError::HandshakeRejected { status: 429 }),
            "the fifth exec session of one session"
        );
        for mut client in held {
            client.close().await;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Nothing in or out for the idle limit ends the session and closes the shell.
        let mut idle = fixture.open(&fixture.access, "manager").await;
        opened(&fixture.agent).await;
        let ExecMessage::Exit(exit) = next(&mut idle).await.unwrap() else {
            panic!("an exit");
        };
        assert_eq!((exit.code, exit.reason.as_str()), (None, "idle_timeout"));
        wait_for(&fixture.agent, 1).await;
        assert_eq!(
            fixture.agent.received().last(),
            Some(&AgentExecInput::Close)
        );

        // A client that stops reading is dropped once it stalls past the limit.
        let mut stalled = fixture.open(&fixture.access, "manager").await;
        opened(&fixture.agent).await;
        let events = fixture.agent.events();
        for _ in 0..2_000 {
            if events
                .send(AgentExecEvent::Output(vec![b'x'; 32 * 1024]))
                .is_err()
            {
                break;
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
        let closed = loop {
            match next(&mut stalled).await {
                Ok(_) => {}
                Err(error) => break error,
            }
        };
        assert!(
            matches!(closed, StreamClientError::Closed { code: 1013, .. }),
            "{closed:?}"
        );

        // An access credential that expires mid-session ends it with the code, then 4401.
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
        let fixture_idle = fixture.context.shared_store.get::<ExecStreams>().unwrap();
        assert!(fixture_idle.timing.idle < Duration::from_secs(1));
        let mut expired = fixture.open(&expiring, "manager").await;
        opened(&fixture.agent).await;
        // Keep it busy so the idle limit doesn't win.
        let keep = tokio::spawn({
            let events = fixture.agent.events();
            async move {
                for _ in 0..40 {
                    let _ = events.send(AgentExecEvent::Output(b".".to_vec()));
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        });
        let error = loop {
            if let ExecMessage::Error(error) = next(&mut expired).await.unwrap() {
                break error;
            }
        };
        assert_eq!(error.code, "session_expired");
        assert_eq!(
            next(&mut expired).await.err(),
            Some(StreamClientError::Closed {
                code: 4401,
                reason: "access expired".to_owned()
            })
        );
        keep.abort();

        fixture.finish().await;
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}
