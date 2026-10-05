//! Services end to end against real PostgreSQL and a fake Swarm manager.

use std::{
    error::Error,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use chrono::Utc;
use kontainer_engine::{ServiceMode, ServiceSummary, TaskSummary};
use loco_rs::{
    app::{AppContext, Hooks},
    config::Config,
    environment::Environment,
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

use crate::{
    app::App,
    auth::{actions::create_session, domain::PrincipalContext},
    operations::{
        actions::get,
        domain::{OperationSnapshot, OperationState},
        executor::ExecutorRegistry,
        worker::{Worker, WorkerSettings},
    },
    services::{domain::ServiceAction, operations::converge::ServiceConvergeExecutor},
    swarm::engine::{ManagerEngine, ManagerError, SharedManagerEngine},
    test_support::PostgresTestDatabase,
};

const WEB: &str = "webappweb0000000000000000";
const AGENT: &str = "monitoring000000000000000";
/// Swarm's message when a task of an update fails.
const PAUSED_MESSAGE: &str = "update paused due to failure or early termination of task x1";

fn task(service: &str, slot: Option<u64>, state: &str, updated: i64) -> TaskSummary {
    TaskSummary {
        id: format!(
            "{service:.20}{:05}",
            slot.unwrap_or(0) + u64::try_from(updated.rem_euclid(1000)).unwrap_or(0)
        ),
        service_id: service.to_owned(),
        slot,
        node_id: Some("swarmnode".to_owned()),
        state: state.to_owned(),
        desired_state: "running".to_owned(),
        updated_at_seconds: Some(updated),
        ..TaskSummary::default()
    }
}

/// A manager whose tasks and update status move one step per `list_tasks` call.
#[derive(Debug)]
struct FakeSwarm {
    services: Mutex<Vec<ServiceSummary>>,
    tasks: Mutex<Vec<TaskSummary>>,
    /// Scheduling fails: new tasks stay pending with this error.
    stuck: AtomicBool,
    /// Updates and rollbacks pause on a failed task instead of completing.
    failing_updates: AtomicBool,
    quorum_lost: AtomicBool,
    /// The next write is accepted, then the swarm loses quorum.
    quorum_lost_after_write: AtomicBool,
    /// The host was demoted: Docker refuses every call as "not a swarm manager".
    demoted: AtomicBool,
    writes: AtomicU64,
}

impl FakeSwarm {
    fn new() -> Self {
        let now = Utc::now().timestamp() - 3600;
        Self {
            services: Mutex::new(vec![
                ServiceSummary {
                    id: WEB.to_owned(),
                    name: "webapp_web".to_owned(),
                    image: format!("nginx:alpine@sha256:{}", "a".repeat(64)),
                    mode: ServiceMode::Replicated { replicas: 1 },
                    ports: vec!["8080:80/tcp".to_owned()],
                    created_at_seconds: Some(1_790_000_000),
                    updated_at_seconds: Some(1_790_000_100),
                    version: 7,
                    stack: Some("webapp".to_owned()),
                    ..ServiceSummary::default()
                },
                ServiceSummary {
                    id: AGENT.to_owned(),
                    name: "monitoring_collector".to_owned(),
                    image: "traefik/whoami".to_owned(),
                    mode: ServiceMode::Global,
                    version: 3,
                    stack: Some("monitoring".to_owned()),
                    ..ServiceSummary::default()
                },
            ]),
            tasks: Mutex::new(vec![
                task(WEB, Some(1), "running", now),
                task(AGENT, None, "running", now),
                task(AGENT, None, "running", now),
                task(AGENT, None, "running", now),
            ]),
            stuck: AtomicBool::new(false),
            failing_updates: AtomicBool::new(false),
            quorum_lost: AtomicBool::new(false),
            quorum_lost_after_write: AtomicBool::new(false),
            demoted: AtomicBool::new(false),
            writes: AtomicU64::new(0),
        }
    }

    fn check(&self) -> Result<(), ManagerError> {
        if self.demoted.load(Ordering::SeqCst) {
            Err(ManagerError::NotAManager)
        } else if self.quorum_lost.load(Ordering::SeqCst) {
            Err(ManagerError::QuorumUnavailable)
        } else {
            Ok(())
        }
    }

    fn bump(
        &self,
        service_id: &str,
        version: u64,
        change: impl FnOnce(&mut ServiceSummary),
    ) -> Result<(), ManagerError> {
        self.check()?;
        let mut services = self.services.lock().unwrap();
        let service = services
            .iter_mut()
            .find(|service| service.id == service_id)
            .ok_or(ManagerError::NotFound)?;
        if service.version != version {
            return Err(ManagerError::VersionConflict);
        }
        change(service);
        service.version += 1;
        self.writes.fetch_add(1, Ordering::SeqCst);
        if self.quorum_lost_after_write.swap(false, Ordering::SeqCst) {
            self.quorum_lost.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    /// One scheduling step for the web service.
    fn step(&self) {
        let desired = {
            let mut services = self.services.lock().unwrap();
            let web = &mut services[0];
            let failing = self.failing_updates.load(Ordering::SeqCst);
            let next = match (web.update_state.as_deref(), failing) {
                (Some("updating"), true) => Some(("paused", PAUSED_MESSAGE)),
                (Some("updating"), false) => Some(("completed", "update completed")),
                (Some("rollback_started"), true) => Some(("rollback_paused", PAUSED_MESSAGE)),
                (Some("rollback_started"), false) => {
                    Some(("rollback_completed", "rollback completed"))
                }
                _ => None,
            };
            if let Some((state, message)) = next {
                web.update_state = Some(state.to_owned());
                web.update_message = Some(message.to_owned());
            }
            match web.mode {
                ServiceMode::Replicated { replicas } => replicas,
                ServiceMode::Global => 0,
            }
        };
        let mut tasks = self.tasks.lock().unwrap();
        if !self.stuck.load(Ordering::SeqCst) {
            for task in tasks.iter_mut().filter(|task| task.state == "pending") {
                task.state = "running".to_owned();
                task.error = None;
            }
        }
        // A task Swarm marked to go stops one step later.
        for task in tasks
            .iter_mut()
            .filter(|task| task.state == "running" && task.desired_state != "running")
        {
            task.state = "shutdown".to_owned();
        }
        let web: Vec<usize> = tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| task.service_id == WEB && task.desired_state == "running")
            .map(|(index, _)| index)
            .collect();
        let now = Utc::now().timestamp();
        if (web.len() as u64) < desired {
            let slot = web
                .iter()
                .filter_map(|index| tasks[*index].slot)
                .max()
                .unwrap_or(0)
                + 1;
            if self.stuck.load(Ordering::SeqCst) {
                if !tasks
                    .iter()
                    .any(|task| task.slot == Some(slot) && task.service_id == WEB)
                {
                    let mut pending = task(WEB, Some(slot), "pending", now);
                    pending.error =
                        Some("no suitable node (insufficient memory on 3 nodes)".to_owned());
                    tasks.push(pending);
                }
            } else {
                tasks.push(task(WEB, Some(slot), "running", now));
            }
        } else if let Some(&lowest) = web.iter().min_by_key(|index| tasks[**index].slot)
            && web.len() as u64 > desired
        {
            // Swarm picks which slot goes; here the lowest, so higher slots stay.
            tasks[lowest].desired_state = "remove".to_owned();
        }
    }
}

#[async_trait::async_trait]
impl ManagerEngine for FakeSwarm {
    async fn list_networks(
        &self,
        _: &CancellationToken,
    ) -> Result<Vec<kontainer_engine::NetworkSummary>, ManagerError> {
        Ok(Vec::new())
    }

    async fn create_network(
        &self,
        _: &kontainer_engine::NetworkCreate,
        _: &CancellationToken,
    ) -> Result<String, ManagerError> {
        Err(ManagerError::Unavailable)
    }

    async fn remove_network(&self, _: &str, _: &CancellationToken) -> Result<(), ManagerError> {
        Err(ManagerError::Unavailable)
    }

    async fn list_services(
        &self,
        _: &CancellationToken,
    ) -> Result<Vec<ServiceSummary>, ManagerError> {
        self.check()?;
        Ok(self.services.lock().unwrap().clone())
    }

    async fn list_tasks(
        &self,
        ids: &[String],
        current_only: bool,
        _: &CancellationToken,
    ) -> Result<Vec<TaskSummary>, ManagerError> {
        self.check()?;
        self.step();
        Ok(self
            .tasks
            .lock()
            .unwrap()
            .iter()
            .filter(|task| ids.is_empty() || ids.contains(&task.service_id))
            .filter(|task| !current_only || task.desired_state == "running")
            .cloned()
            .collect())
    }

    async fn scale_service(
        &self,
        id: &str,
        replicas: u64,
        version: u64,
        _: &CancellationToken,
    ) -> Result<(), ManagerError> {
        self.bump(id, version, |service| {
            service.mode = ServiceMode::Replicated { replicas }
        })
    }

    async fn force_update_service(
        &self,
        id: &str,
        version: u64,
        _: &CancellationToken,
    ) -> Result<(), ManagerError> {
        // Swarm clears the status on the write; its updater reports `updating` a moment later.
        self.bump(id, version, |service| {
            service.update_state = Some("updating".to_owned());
            service.update_message = Some("update in progress".to_owned());
        })
    }

    async fn rollback_service(
        &self,
        id: &str,
        version: u64,
        _: &CancellationToken,
    ) -> Result<(), ManagerError> {
        self.bump(id, version, |service| {
            service.update_state = Some("rollback_started".to_owned());
            service.update_message = Some("manually requested rollback".to_owned());
        })
    }

    async fn swarm_status(&self, _: &CancellationToken) -> Result<(), ManagerError> {
        self.check()
    }
}

struct Fixture {
    router: Router,
    context: AppContext,
    swarm: Arc<FakeSwarm>,
    authorization: String,
    principal: PrincipalContext,
}

async fn fixture(
    database: &sea_orm::DatabaseConnection,
    with_manager: bool,
) -> Result<Fixture, Box<dyn Error>> {
    let swarm = Arc::new(FakeSwarm::new());
    let config = Config::from_folder(&Environment::Test, Path::new("config"))?;
    let context = App::after_context(
        AppContext::builder(Environment::Test, database.clone(), config).build(),
    )
    .await?;
    if with_manager {
        context
            .shared_store
            .insert(SharedManagerEngine(swarm.clone() as Arc<dyn ManagerEngine>));
    }
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
    let session = create_session::execute(database, principal).await?;
    let router = App::routes(&context).to_router::<App>(context.clone(), Router::new())?;
    Ok(Fixture {
        router,
        context,
        swarm,
        authorization: format!("Bearer {}", session.access_plaintext),
        principal: PrincipalContext::new(principal),
    })
}

async fn send(
    fixture: &Fixture,
    method: &str,
    path: &str,
    key: Option<&str>,
    body: &str,
) -> Result<(StatusCode, Option<String>, serde_json::Value), Box<dyn Error>> {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, &fixture.authorization)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(key) = key {
        request = request.header("idempotency-key", key);
    }
    let response = fixture
        .router
        .clone()
        .oneshot(request.body(Body::from(body.to_owned()))?)
        .await?;
    let status = response.status();
    let retry = response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let bytes = to_bytes(response.into_body(), 1 << 20).await?;
    Ok((
        status,
        retry,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    ))
}

/// Run one operation with the service executors polling every 20 ms for at most `deadline`.
async fn run(
    fixture: &Fixture,
    operation_id: &str,
    deadline: Duration,
) -> Result<OperationSnapshot, Box<dyn Error>> {
    let fast = |action| {
        ServiceConvergeExecutor::new(action).with_timing(Duration::from_millis(20), deadline)
    };
    let registry = ExecutorRegistry::default()
        .with(fast(ServiceAction::Scale))
        .with(fast(ServiceAction::Restart))
        .with(fast(ServiceAction::Rollback));
    let draining = CancellationToken::new();
    let worker = Worker {
        database: fixture.context.db.clone(),
        registry: Arc::new(registry),
        agents: None,
        manager: Some(fixture.swarm.clone() as Arc<dyn ManagerEngine>),
        settings: WorkerSettings {
            idle_min: Duration::from_millis(10),
            idle_max: Duration::from_millis(20),
            ..WorkerSettings::new(Duration::from_secs(2))
        },
    };
    let task = tokio::spawn(worker.run(draining.clone()));
    let id = operation_id.parse()?;
    let mut snapshot = get::execute(&fixture.context.db, &fixture.principal, id).await?;
    for _ in 0..400 {
        if snapshot.operation.state.is_terminal() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
        snapshot = get::execute(&fixture.context.db, &fixture.principal, id).await?;
    }
    draining.cancel();
    task.await?;
    Ok(snapshot)
}

fn key(n: u32) -> String {
    format!("018f2e34-0000-7000-8000-{n:012}")
}

#[tokio::test]
async fn postgres_services_list_counts_detail_and_quorum() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection, true).await?;
        let (status, _, body) = send(&fixture, "GET", "/api/v1/services", None, "").await?;
        assert_eq!(status, StatusCode::OK);
        let rows: Vec<_> = body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                (
                    item["name"].as_str().unwrap(),
                    item["mode"].as_str().unwrap(),
                    item["runningReplicas"].as_u64().unwrap(),
                    item["desiredReplicas"].as_u64().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            vec![
                ("monitoring_collector", "global", 3, 3),
                ("webapp_web", "replicated", 1, 1)
            ]
        );
        let web = &body["items"][1];
        assert_eq!(web["image"], "nginx:alpine", "shown without the digest");
        assert!(web["imageReference"].as_str().unwrap().contains("@sha256:"));
        assert_eq!(
            (web["version"].as_u64(), web["stack"].as_str()),
            (Some(7), Some("webapp"))
        );
        let (_, _, stack) = send(
            &fixture,
            "GET",
            "/api/v1/services?stack=monitoring",
            None,
            "",
        )
        .await?;
        assert_eq!(stack["items"].as_array().unwrap().len(), 1);

        fixture.swarm.stuck.store(true, Ordering::SeqCst);
        fixture
            .swarm
            .scale_service(WEB, 2, 7, &CancellationToken::new())
            .await
            .unwrap();
        let (status, _, detail) = send(
            &fixture,
            "GET",
            &format!("/api/v1/services/{WEB}"),
            None,
            "",
        )
        .await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            detail,
            serde_json::json!({"latestError": "no suitable node (insufficient memory on 3 nodes)"}),
            "only what the replica tooltip reads"
        );
        let (status, _, _) = send(&fixture, "GET", "/api/v1/services/nope", None, "").await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        fixture.swarm.quorum_lost.store(true, Ordering::SeqCst);
        let (status, retry, error) = send(&fixture, "GET", "/api/v1/services", None, "").await?;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            error,
            serde_json::json!({"error": "quorum_unavailable", "retryAfterSeconds": 10})
        );
        assert_eq!(retry.as_deref(), Some("10"));
        let writes = fixture.swarm.writes.load(Ordering::SeqCst);
        let (status, _, error) = send(
            &fixture,
            "POST",
            &format!("/api/v1/services/{WEB}/actions/restart"),
            Some(&key(601)),
            r#"{"version":8}"#,
        )
        .await?;
        assert_eq!(
            (status, error["error"].as_str()),
            (StatusCode::SERVICE_UNAVAILABLE, Some("quorum_unavailable"))
        );
        assert!(error.get("operationId").is_none(), "nothing admitted");
        assert_eq!(fixture.swarm.writes.load(Ordering::SeqCst), writes);

        // Demoted since: the engine exists, and Docker says it isn't a manager.
        fixture.swarm.demoted.store(true, Ordering::SeqCst);
        let (status, _, error) = send(&fixture, "GET", "/api/v1/services", None, "").await?;
        assert_eq!(
            (status, error["error"].as_str()),
            (StatusCode::SERVICE_UNAVAILABLE, Some("not_a_manager"))
        );

        let without = fixture_without_manager(&database.connection).await?;
        let (status, _, error) = send(&without, "GET", "/api/v1/services", None, "").await?;
        assert_eq!(
            (status, error["error"].as_str()),
            (StatusCode::SERVICE_UNAVAILABLE, Some("not_a_manager"))
        );
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

async fn fixture_without_manager(
    database: &sea_orm::DatabaseConnection,
) -> Result<Fixture, Box<dyn Error>> {
    fixture(database, false).await
}

#[tokio::test]
async fn postgres_service_actions_validate_and_converge() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection, true).await?;
        for (path, body, status, field) in [
            (
                format!("/api/v1/services/{WEB}/actions/scale"),
                r#"{"version":7}"#,
                StatusCode::BAD_REQUEST,
                Some("replicas"),
            ),
            (
                format!("/api/v1/services/{WEB}/actions/scale"),
                r#"{"version":7,"replicas":1001}"#,
                StatusCode::BAD_REQUEST,
                Some("replicas"),
            ),
            (
                format!("/api/v1/services/{WEB}/actions/restart"),
                r#"{"version":7,"replicas":2}"#,
                StatusCode::BAD_REQUEST,
                Some("replicas"),
            ),
            (
                format!("/api/v1/services/{AGENT}/actions/scale"),
                r#"{"version":3,"replicas":2}"#,
                StatusCode::BAD_REQUEST,
                Some("replicas"),
            ),
            (
                format!("/api/v1/services/{WEB}/actions/explode"),
                r#"{"version":7}"#,
                StatusCode::NOT_FOUND,
                None,
            ),
            (
                format!("/api/v1/services/{WEB}/actions/scale"),
                r#"{"version":6,"replicas":2}"#,
                StatusCode::CONFLICT,
                None,
            ),
        ] {
            let (actual, _, error) = send(&fixture, "POST", &path, Some(&key(610)), body).await?;
            assert_eq!(actual, status, "{path} {body}");
            if let Some(field) = field {
                assert!(error["fields"][field].is_string(), "{path} {body}: {error}");
            }
        }

        // Up: accepted, then converging, then converged.
        let (status, _, admitted) = send(
            &fixture,
            "POST",
            &format!("/api/v1/services/{WEB}/actions/scale"),
            Some(&key(611)),
            r#"{"version":7,"replicas":3}"#,
        )
        .await?;
        assert_eq!(status, StatusCode::ACCEPTED);
        let up = run(
            &fixture,
            admitted["operationId"].as_str().unwrap(),
            Duration::from_secs(2),
        )
        .await?;
        assert_eq!(
            up.operation.state,
            OperationState::Succeeded,
            "{:?}",
            up.error_message
        );
        assert_eq!(
            up.progress.unwrap(),
            serde_json::json!({"desired": 3, "running": 3, "phase": "converged"})
        );
        // A retry whose response was lost replays, though version 7 is stale by now.
        let (status, _, replay) = send(
            &fixture,
            "POST",
            &format!("/api/v1/services/{WEB}/actions/scale"),
            Some(&key(611)),
            r#"{"version":7,"replicas":3}"#,
        )
        .await?;
        assert_eq!(
            (status, &replay["operationId"], &replay["replayed"]),
            (
                StatusCode::ACCEPTED,
                &admitted["operationId"],
                &serde_json::json!(true)
            )
        );
        // The same key for another action is not a replay.
        let (status, _, error) = send(
            &fixture,
            "POST",
            &format!("/api/v1/services/{WEB}/actions/restart"),
            Some(&key(611)),
            r#"{"version":7}"#,
        )
        .await?;
        assert_eq!(
            (status, error["error"].as_str()),
            (StatusCode::CONFLICT, Some("idempotency_conflict"))
        );

        // Down to one.
        let (_, _, admitted) = send(
            &fixture,
            "POST",
            &format!("/api/v1/services/{WEB}/actions/scale"),
            Some(&key(612)),
            r#"{"version":8,"replicas":1}"#,
        )
        .await?;
        let down = run(
            &fixture,
            admitted["operationId"].as_str().unwrap(),
            Duration::from_secs(2),
        )
        .await?;
        assert_eq!(
            down.operation.state,
            OperationState::Succeeded,
            "{:?}",
            down.error_message
        );
        assert_eq!(down.progress.unwrap()["running"], 1);
        let kept: Vec<_> = fixture
            .swarm
            .tasks
            .lock()
            .unwrap()
            .iter()
            .filter(|task| task.service_id == WEB && task.is_running())
            .map(|task| task.slot)
            .collect();
        assert_eq!(kept, vec![Some(3)], "the slot kept is above the new count");

        // Scheduling fails: the timeout names the task error.
        fixture.swarm.stuck.store(true, Ordering::SeqCst);
        let (_, _, admitted) = send(
            &fixture,
            "POST",
            &format!("/api/v1/services/{WEB}/actions/scale"),
            Some(&key(613)),
            r#"{"version":9,"replicas":2}"#,
        )
        .await?;
        let stuck = run(
            &fixture,
            admitted["operationId"].as_str().unwrap(),
            Duration::from_millis(200),
        )
        .await?;
        assert_eq!(stuck.operation.state, OperationState::Failed);
        assert_eq!(stuck.last_error_code.as_deref(), Some("converge_timeout"));
        let message = stuck.error_message.unwrap();
        assert!(message.contains("1 of 2 running"), "{message}");
        assert!(
            message.contains("no suitable node (insufficient memory on 3 nodes)"),
            "{message}"
        );
        fixture.swarm.stuck.store(false, Ordering::SeqCst);

        // Restart and rollback end on Swarm's update status.
        let (_, _, admitted) = send(
            &fixture,
            "POST",
            &format!("/api/v1/services/{WEB}/actions/restart"),
            Some(&key(614)),
            r#"{"version":10}"#,
        )
        .await?;
        let restart = run(
            &fixture,
            admitted["operationId"].as_str().unwrap(),
            Duration::from_secs(2),
        )
        .await?;
        assert_eq!(
            restart.operation.state,
            OperationState::Succeeded,
            "{:?}",
            restart.error_message
        );
        let (_, _, admitted) = send(
            &fixture,
            "POST",
            &format!("/api/v1/services/{WEB}/actions/rollback"),
            Some(&key(615)),
            r#"{"version":11}"#,
        )
        .await?;
        let rollback = run(
            &fixture,
            admitted["operationId"].as_str().unwrap(),
            Duration::from_secs(2),
        )
        .await?;
        assert_eq!(
            rollback.operation.state,
            OperationState::Succeeded,
            "{:?}",
            rollback.error_message
        );
        assert_eq!(rollback.progress.unwrap()["phase"], "converged");

        // A rollback Swarm pauses on a failed task fails with Swarm's own message.
        fixture.swarm.failing_updates.store(true, Ordering::SeqCst);
        let (_, _, admitted) = send(
            &fixture,
            "POST",
            &format!("/api/v1/services/{WEB}/actions/rollback"),
            Some(&key(616)),
            r#"{"version":12}"#,
        )
        .await?;
        let paused = run(
            &fixture,
            admitted["operationId"].as_str().unwrap(),
            Duration::from_secs(2),
        )
        .await?;
        assert_eq!(paused.operation.state, OperationState::Failed);
        assert_eq!(paused.last_error_code.as_deref(), Some("rollback_paused"));
        let message = paused.error_message.unwrap();
        assert_eq!(
            message,
            format!("Swarm paused the rollback of webapp_web: {PAUSED_MESSAGE}.")
        );
        fixture.swarm.failing_updates.store(false, Ordering::SeqCst);

        // Quorum goes right after the restart is accepted: the failure says so, not "0 of 0".
        fixture
            .swarm
            .quorum_lost_after_write
            .store(true, Ordering::SeqCst);
        let (_, _, admitted) = send(
            &fixture,
            "POST",
            &format!("/api/v1/services/{WEB}/actions/restart"),
            Some(&key(617)),
            r#"{"version":13}"#,
        )
        .await?;
        let unread = run(
            &fixture,
            admitted["operationId"].as_str().unwrap(),
            Duration::from_millis(200),
        )
        .await?;
        assert_eq!(unread.operation.state, OperationState::Failed);
        assert_eq!(
            unread.last_error_code.as_deref(),
            Some("quorum_unavailable")
        );
        let message = unread.error_message.unwrap();
        assert!(
            message.starts_with("webapp_web was changed, but the manager couldn't be read"),
            "{message}"
        );
        fixture.swarm.quorum_lost.store(false, Ordering::SeqCst);
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}
