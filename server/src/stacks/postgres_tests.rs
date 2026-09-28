//! Stacks end to end against real PostgreSQL, a fake Swarm manager, and one fake agent.

use std::{
    collections::BTreeMap,
    error::Error,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use chrono::Utc;
use kontainer_engine::{NetworkSummary, ServiceMode, ServiceSummary, TaskSummary};
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
    nodes::{
        actions::record_check_in,
        agents::{AgentDirectory, NodeEndpoint, SharedAgentDirectory},
        postgres_tests::check_in,
    },
    operations::{
        actions::get,
        domain::{OperationSnapshot, OperationState},
        executor::ExecutorRegistry,
        worker::{Worker, WorkerSettings},
    },
    rpc::{
        AgentClient, AgentClientError, AgentContainerPage, AgentContainerState,
        AgentContainerSummary, AgentDescription,
    },
    stacks::operations::remove::RemoveStackExecutor,
    swarm::engine::{ManagerEngine, ManagerError, SharedManagerEngine},
    test_support::PostgresTestDatabase,
};

fn swarm_id(prefix: &str) -> String {
    format!("{prefix:0<25}")
}

fn service(
    name: &str,
    stack: &str,
    replicas: u64,
    volumes: &[&str],
    updated: i64,
) -> ServiceSummary {
    ServiceSummary {
        id: swarm_id(&name.replace('_', "")),
        name: name.to_owned(),
        image: "nginx:alpine".to_owned(),
        mode: ServiceMode::Replicated { replicas },
        stack: Some(stack.to_owned()),
        updated_at_seconds: Some(updated),
        version: 1,
        volumes: volumes.iter().map(|volume| (*volume).to_owned()).collect(),
        ..ServiceSummary::default()
    }
}

fn running(service: &ServiceSummary, slot: u64) -> TaskSummary {
    TaskSummary {
        id: format!("{:.20}{slot:05}", service.id),
        service_id: service.id.clone(),
        slot: Some(slot),
        state: "running".to_owned(),
        desired_state: "running".to_owned(),
        ..TaskSummary::default()
    }
}

fn stack_network(name: &str, stack: &str) -> NetworkSummary {
    NetworkSummary {
        id: swarm_id(&name.replace('_', "")),
        name: name.to_owned(),
        driver: "overlay".to_owned(),
        scope: "swarm".to_owned(),
        labels: BTreeMap::from([("com.docker.stack.namespace".to_owned(), stack.to_owned())]),
        ..NetworkSummary::default()
    }
}

/// Tasks of removed services linger for `linger` task listings; a network refuses removal
/// `busy` times first.
#[derive(Debug)]
struct FakeSwarm {
    services: Mutex<Vec<ServiceSummary>>,
    tasks: Mutex<Vec<TaskSummary>>,
    networks: Mutex<Vec<NetworkSummary>>,
    linger: AtomicU32,
    busy: AtomicU32,
    log: Mutex<Vec<String>>,
}

impl FakeSwarm {
    fn new() -> Self {
        let web = service("webapp_web", "webapp", 3, &[], 100);
        let api = service("webapp_api", "webapp", 2, &[], 300);
        let nginx = service("toolbox_nginx", "toolbox", 2, &[], 50);
        let redis = service("toolbox_redis", "toolbox", 1, &["toolbox_redis-data"], 60);
        let tasks = vec![
            running(&web, 1),
            running(&web, 2),
            running(&web, 3),
            running(&api, 1),
            running(&nginx, 1),
            running(&nginx, 2),
            running(&redis, 1),
        ];
        Self {
            services: Mutex::new(vec![web, api, nginx, redis]),
            tasks: Mutex::new(tasks),
            networks: Mutex::new(vec![
                stack_network("toolbox_kontainer", "toolbox"),
                stack_network("webapp_kontainer", "webapp"),
                NetworkSummary {
                    id: swarm_id("ingress"),
                    name: "ingress".to_owned(),
                    scope: "swarm".to_owned(),
                    ..NetworkSummary::default()
                },
            ]),
            linger: AtomicU32::new(2),
            busy: AtomicU32::new(1),
            log: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait::async_trait]
impl ManagerEngine for FakeSwarm {
    async fn list_networks(
        &self,
        _: &CancellationToken,
    ) -> Result<Vec<NetworkSummary>, ManagerError> {
        Ok(self.networks.lock().unwrap().clone())
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
        Err(ManagerError::Unavailable)
    }

    async fn remove_network(&self, id: &str, _: &CancellationToken) -> Result<(), ManagerError> {
        let services = self.services.lock().unwrap().clone();
        let orphaned = self
            .tasks
            .lock()
            .unwrap()
            .iter()
            .any(|task| !services.iter().any(|service| service.id == task.service_id));
        if orphaned {
            return Err(ManagerError::Rejected);
        }
        if self.busy.load(Ordering::SeqCst) > 0 {
            self.busy.fetch_sub(1, Ordering::SeqCst);
            self.log.lock().unwrap().push("network busy".to_owned());
            return Err(ManagerError::Rejected);
        }
        let mut networks = self.networks.lock().unwrap();
        let name = networks
            .iter()
            .find(|network| network.id == id)
            .map(|network| network.name.clone());
        networks.retain(|network| network.id != id);
        self.log
            .lock()
            .unwrap()
            .push(format!("remove network {}", name.unwrap_or_default()));
        Ok(())
    }

    async fn list_services(
        &self,
        _: &CancellationToken,
    ) -> Result<Vec<ServiceSummary>, ManagerError> {
        Ok(self.services.lock().unwrap().clone())
    }

    async fn list_tasks(
        &self,
        ids: &[String],
        _: bool,
        _: &CancellationToken,
    ) -> Result<Vec<TaskSummary>, ManagerError> {
        let services = self.services.lock().unwrap();
        let mut tasks = self.tasks.lock().unwrap();
        // Tasks of removed services go after a few looks.
        let orphaned = tasks
            .iter()
            .any(|task| !services.iter().any(|service| service.id == task.service_id));
        if orphaned {
            if self.linger.load(Ordering::SeqCst) == 0 {
                tasks.retain(|task| services.iter().any(|service| service.id == task.service_id));
            } else {
                self.linger.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let listed: Vec<TaskSummary> = tasks
            .iter()
            .filter(|task| ids.contains(&task.service_id))
            .cloned()
            .collect();
        // Like Docker: a filter naming only removed services is a 404 once their tasks are gone.
        let known = ids
            .iter()
            .any(|id| services.iter().any(|service| &service.id == id));
        if listed.is_empty() && !known {
            return Err(ManagerError::NotFound);
        }
        Ok(listed)
    }

    async fn remove_service(&self, id: &str, _: &CancellationToken) -> Result<(), ManagerError> {
        let mut services = self.services.lock().unwrap();
        let name = services
            .iter()
            .find(|service| service.id == id)
            .map(|service| service.name.clone());
        services.retain(|service| service.id != id);
        self.log
            .lock()
            .unwrap()
            .push(format!("remove service {}", name.unwrap_or_default()));
        Ok(())
    }
}

#[derive(Debug)]
struct FakeAgent;

#[async_trait::async_trait]
impl AgentClient for FakeAgent {
    async fn describe(&self, _: &CancellationToken) -> Result<AgentDescription, AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    async fn list_containers(
        &self,
        _: u32,
        _: String,
        _: &CancellationToken,
    ) -> Result<AgentContainerPage, AgentClientError> {
        let container = |fill: char, stack: &str, cpu: f64, memory: u64| AgentContainerSummary {
            container_id: fill.to_string().repeat(64),
            display_name: format!("{stack}_x.{fill}"),
            image_reference: "nginx:alpine".to_owned(),
            state: AgentContainerState::Running,
            status: "Up".to_owned(),
            created_at_seconds: 1_790_000_000,
            node_id: "manager".to_owned(),
            started_at_seconds: None,
            finished_at_seconds: None,
            published_ports: Vec::new(),
            service_name: None,
            stack_namespace: Some(stack.to_owned()),
            task_id: None,
            cpu_percent: Some(cpu),
            memory_used_bytes: Some(memory),
            memory_limit_bytes: None,
            networks: Vec::new(),
        };
        Ok(AgentContainerPage {
            containers: vec![
                container('a', "webapp", 10.0, 1 << 20),
                container('b', "webapp", 2.5, 1 << 20),
                container('c', "toolbox", 1.0, 1 << 10),
            ],
            next_page_token: None,
        })
    }

    fn node_id(&self) -> &str {
        "manager"
    }
}

#[derive(Debug)]
struct FakeDirectory;

impl AgentDirectory for FakeDirectory {
    fn client(&self, _: &NodeEndpoint) -> Result<Arc<dyn AgentClient>, AgentClientError> {
        Ok(Arc::new(FakeAgent))
    }
}

struct Fixture {
    router: Router,
    context: AppContext,
    swarm: Arc<FakeSwarm>,
    authorization: String,
    principal: PrincipalContext,
}

async fn fixture(database: &sea_orm::DatabaseConnection) -> Result<Fixture, Box<dyn Error>> {
    record_check_in::execute(database, "test", check_in("manager", None)).await?;
    let swarm = Arc::new(FakeSwarm::new());
    let config = Config::from_folder(&Environment::Test, Path::new("config"))?;
    let context = App::after_context(
        AppContext::builder(Environment::Test, database.clone(), config).build(),
    )
    .await?;
    context
        .shared_store
        .insert(SharedManagerEngine(swarm.clone() as Arc<dyn ManagerEngine>));
    context.shared_store.insert(SharedAgentDirectory(
        Arc::new(FakeDirectory) as Arc<dyn AgentDirectory>
    ));
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
) -> Result<(StatusCode, serde_json::Value), Box<dyn Error>> {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, &fixture.authorization);
    if let Some(key) = key {
        request = request.header("idempotency-key", key);
    }
    let response = fixture
        .router
        .clone()
        .oneshot(request.body(Body::empty())?)
        .await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1 << 20).await?;
    Ok((
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    ))
}

async fn run(
    fixture: &Fixture,
    operation_id: &str,
    deadline: Duration,
) -> Result<OperationSnapshot, Box<dyn Error>> {
    let registry = ExecutorRegistry::default().with(RemoveStackExecutor::with_timing(
        Duration::from_millis(20),
        deadline,
    ));
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

#[tokio::test]
async fn postgres_stacks_group_services_and_plan_their_removal() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection).await?;
        let (status, body) = send(&fixture, "GET", "/api/v1/stacks", None).await?;
        assert_eq!(status, StatusCode::OK);
        let rows: Vec<_> = body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                (
                    item["name"].as_str().unwrap(),
                    item["status"].as_str().unwrap(),
                    item["services"].as_u64().unwrap(),
                    item["runningContainers"].as_u64().unwrap(),
                    item["desiredContainers"].as_u64().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            vec![
                ("toolbox", "running", 2, 3, 3),
                ("webapp", "partial", 2, 4, 5)
            ]
        );
        let webapp = &body["items"][1];
        assert_eq!(
            (
                webapp["cpuPercent"].as_f64(),
                webapp["memoryBytes"].as_u64()
            ),
            (Some(12.5), Some(2 << 20))
        );
        assert_eq!(webapp["lastDeployed"], "1970-01-01T00:05:00.000Z");

        let (status, plan) = send(&fixture, "GET", "/api/v1/stacks/toolbox/plan", None).await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            plan,
            serde_json::json!({
                "services": ["toolbox_nginx", "toolbox_redis"],
                "networks": ["toolbox_kontainer"],
                "volumesRetained": ["toolbox_redis-data"],
            })
        );
        let (status, _) = send(&fixture, "GET", "/api/v1/stacks/ghost/plan", None).await?;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = send(&fixture, "GET", "/api/v1/stacks/-bad/plan", None).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_stack_removal_goes_services_then_networks_and_keeps_volumes()
-> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection).await?;
        let (status, admitted) = send(&fixture, "DELETE", "/api/v1/stacks/toolbox", Some("018f2e34-0000-7000-8000-000000000701")).await?;
        assert_eq!(status, StatusCode::ACCEPTED);
        let removed = run(&fixture, admitted["operationId"].as_str().unwrap(), Duration::from_secs(2)).await?;
        assert_eq!(removed.operation.state, OperationState::Succeeded, "{:?}", removed.error_message);
        assert_eq!(
            removed.progress.unwrap(),
            serde_json::json!({"phase": "removed", "servicesRemoved": 2, "networksRemoved": 1, "volumesRetained": ["toolbox_redis-data"]})
        );
        assert_eq!(
            *fixture.swarm.log.lock().unwrap(),
            vec!["remove service toolbox_nginx", "remove service toolbox_redis", "network busy", "remove network toolbox_kontainer"],
            "services first, then the network once the tasks are gone, retried while busy"
        );
        let names: Vec<_> = fixture.swarm.services.lock().unwrap().iter().map(|service| service.name.clone()).collect();
        assert_eq!(names, vec!["webapp_web", "webapp_api"], "the other stack is untouched");
        let (status, _) = send(&fixture, "DELETE", "/api/v1/stacks/toolbox", Some("018f2e34-0000-7000-8000-000000000702")).await?;
        assert_eq!(status, StatusCode::NOT_FOUND, "gone");
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_stack_removal_reports_tasks_that_never_go() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection).await?;
        fixture.swarm.linger.store(u32::MAX, Ordering::SeqCst);
        let (_, admitted) = send(
            &fixture,
            "DELETE",
            "/api/v1/stacks/webapp",
            Some("018f2e34-0000-7000-8000-000000000703"),
        )
        .await?;
        let stuck = run(
            &fixture,
            admitted["operationId"].as_str().unwrap(),
            Duration::from_millis(150),
        )
        .await?;
        assert_eq!(stuck.operation.state, OperationState::Failed);
        assert_eq!(stuck.last_error_code.as_deref(), Some("tasks_remaining"));
        assert_eq!(stuck.progress.unwrap()["servicesRemoved"], 2);
        assert!(
            fixture
                .swarm
                .networks
                .lock()
                .unwrap()
                .iter()
                .any(|network| network.name == "webapp_kontainer"),
            "networks left"
        );
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}
