//! The cluster container list and container operations against real PostgreSQL and fake agents.

use std::{
    collections::HashMap,
    error::Error,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use chrono::Utc;
use loco_rs::{
    app::{AppContext, Hooks},
    config::Config,
    environment::Environment,
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;

use crate::{
    app::App,
    auth::{actions::create_session, domain::PrincipalContext},
    nodes::{
        actions::record_check_in,
        agents::{AgentDirectory, NodeEndpoint, SharedAgentDirectory},
        entities::node,
        postgres_tests::check_in,
    },
    operations::{
        actions::get,
        domain::{OperationSnapshot, OperationState},
        executor::production_registry,
        worker::{Worker, WorkerSettings},
    },
    rpc::{
        AgentClient, AgentClientError, AgentContainerAction, AgentContainerPage,
        AgentContainerState, AgentContainerSummary, AgentDescription,
    },
    test_support::PostgresTestDatabase,
};

type ActionLog = Arc<Mutex<Vec<(String, String, AgentContainerAction, bool)>>>;

#[derive(Debug)]
struct FakeAgent {
    node_id: String,
    containers: Vec<AgentContainerSummary>,
    reachable: bool,
    actions: ActionLog,
}

#[async_trait::async_trait]
impl AgentClient for FakeAgent {
    async fn describe(&self, _: &CancellationToken) -> Result<AgentDescription, AgentClientError> {
        Err(AgentClientError::Unavailable)
    }

    async fn list_containers(
        &self,
        page_size: u32,
        page_token: String,
        _: &CancellationToken,
    ) -> Result<AgentContainerPage, AgentClientError> {
        if !self.reachable {
            return Err(AgentClientError::Unavailable);
        }
        let start: usize = page_token.parse().unwrap_or(0);
        let end = (start + page_size as usize).min(self.containers.len());
        Ok(AgentContainerPage {
            containers: self.containers[start..end].to_vec(),
            next_page_token: (end < self.containers.len()).then(|| end.to_string()),
        })
    }

    async fn container_action(
        &self,
        container_id: &str,
        action: AgentContainerAction,
        force: bool,
        _timeout_seconds: Option<u32>,
        _: &CancellationToken,
    ) -> Result<(), AgentClientError> {
        self.actions.lock().unwrap().push((
            self.node_id.clone(),
            container_id.to_owned(),
            action,
            force,
        ));
        if !self.reachable {
            return Err(AgentClientError::Unavailable);
        }
        if container_id.starts_with('b') {
            return Err(AgentClientError::EngineError);
        }
        if action == AgentContainerAction::Remove && !force {
            return Err(AgentClientError::ContainerRunning);
        }
        Ok(())
    }

    fn node_id(&self) -> &str {
        &self.node_id
    }
}

#[derive(Debug)]
struct FakeDirectory(HashMap<String, Arc<FakeAgent>>);

impl AgentDirectory for FakeDirectory {
    fn client(&self, node: &NodeEndpoint) -> Result<Arc<dyn AgentClient>, AgentClientError> {
        self.0
            .get(&node.node_id)
            .cloned()
            .map(|agent| agent as Arc<dyn AgentClient>)
            .ok_or(AgentClientError::Unavailable)
    }
}

fn summary(node_id: &str, fill: char, index: usize) -> AgentContainerSummary {
    AgentContainerSummary {
        container_id: format!("{fill}{index:063x}"),
        display_name: format!("web-{index}"),
        image_reference: "nginx:latest".to_owned(),
        state: AgentContainerState::Running,
        status: "Up 3 minutes".to_owned(),
        created_at_seconds: 1_700_000_000,
        node_id: node_id.to_owned(),
        started_at_seconds: Some(1_700_000_100),
        finished_at_seconds: None,
        published_ports: vec!["8080:80/tcp".to_owned()],
        service_name: Some("shop_web".to_owned()),
        stack_namespace: Some("shop".to_owned()),
        task_id: Some("task-1".to_owned()),
        cpu_percent: Some(12.5),
        memory_used_bytes: Some(1 << 20),
        memory_limit_bytes: Some(1 << 30),
    }
}

struct Fixture {
    router: Router,
    context: AppContext,
    directory: Arc<FakeDirectory>,
    actions: ActionLog,
    authorization: String,
    principal: PrincipalContext,
}

async fn test_principal(
    database: &sea_orm::DatabaseConnection,
) -> Result<uuid::Uuid, Box<dyn Error>> {
    let now = Utc::now();
    let id = uuid::Uuid::now_v7();
    crate::auth::entities::principal::ActiveModel {
        id: Set(id),
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
    Ok(id)
}

async fn fixture(
    database: &sea_orm::DatabaseConnection,
    manager_containers: usize,
) -> Result<Fixture, Box<dyn Error>> {
    for node_id in ["manager", "worker-1", "worker-2"] {
        let mut member = check_in(node_id, None);
        member.hostname = format!("{node_id}.local");
        record_check_in::execute(database, "test", member).await?;
    }
    // worker-2 went silent two minutes ago.
    let row = node::Entity::find_by_id("worker-2")
        .one(database)
        .await?
        .unwrap();
    let mut row: node::ActiveModel = row.into();
    row.last_check_in_at = Set(Utc::now() - chrono::Duration::minutes(2));
    row.update(database).await?;

    let actions: ActionLog = Arc::default();
    let agent = |node_id: &str, containers, reachable| {
        (
            node_id.to_owned(),
            Arc::new(FakeAgent {
                node_id: node_id.to_owned(),
                containers,
                reachable,
                actions: actions.clone(),
            }),
        )
    };
    let directory = Arc::new(FakeDirectory(HashMap::from([
        agent(
            "manager",
            (0..manager_containers)
                .map(|index| summary("manager", 'a', index))
                .collect(),
            true,
        ),
        agent("worker-1", vec![summary("worker-1", 'c', 0)], false),
        agent("worker-2", vec![summary("worker-2", 'd', 0)], true),
    ])));
    let config = Config::from_folder(&Environment::Test, Path::new("config"))?;
    let context = App::after_context(
        AppContext::builder(Environment::Test, database.clone(), config).build(),
    )
    .await?;
    context.shared_store.insert(SharedAgentDirectory(
        directory.clone() as Arc<dyn AgentDirectory>
    ));
    let principal = test_principal(database).await?;
    let session = create_session::execute(database, principal).await?;
    let router = App::routes(&context).to_router::<App>(context.clone(), Router::new())?;
    Ok(Fixture {
        router,
        context,
        directory,
        actions,
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
) -> Result<(StatusCode, serde_json::Value), Box<dyn Error>> {
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
    let bytes = to_bytes(response.into_body(), 4 << 20).await?;
    let json = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes)?
    };
    Ok((status, json))
}

async fn run_until_terminal(
    fixture: &Fixture,
    principal: &PrincipalContext,
    operation_id: &str,
) -> Result<OperationSnapshot, Box<dyn Error>> {
    let draining = CancellationToken::new();
    let worker = Worker {
        database: fixture.context.db.clone(),
        registry: Arc::new(production_registry()),
        agents: Some(fixture.directory.clone() as Arc<dyn AgentDirectory>),
        settings: WorkerSettings {
            idle_min: Duration::from_millis(10),
            idle_max: Duration::from_millis(20),
            ..WorkerSettings::new(Duration::from_secs(2))
        },
    };
    let task = tokio::spawn(worker.run(draining.clone()));
    let id = operation_id.parse()?;
    let mut snapshot = get::execute(&fixture.context.db, principal, id).await?;
    for _ in 0..300 {
        if snapshot.operation.state.is_terminal() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
        snapshot = get::execute(&fixture.context.db, principal, id).await?;
    }
    draining.cancel();
    task.await?;
    Ok(snapshot)
}

#[tokio::test]
async fn postgres_list_merges_answers_and_reports_every_member() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection, 2).await?;
        let (status, body) = send(&fixture, "GET", "/api/v1/containers", None, "").await?;
        assert_eq!(status, StatusCode::OK);
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 2, "only the answering node's containers");
        assert_eq!(items[0]["nodeId"], "manager");
        assert_eq!(items[0]["nodeName"], "manager.local");
        assert_eq!(items[0]["ports"], serde_json::json!(["8080:80/tcp"]));
        assert_eq!(items[0]["serviceName"], "shop_web");
        assert_eq!(items[0]["cpuPercent"], 12.5);
        assert!(items[0]["startedAt"].as_str().unwrap().ends_with('Z'));
        let outcomes: Vec<_> = body["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| {
                (
                    node["nodeId"].as_str().unwrap().to_owned(),
                    node["status"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        assert_eq!(
            outcomes,
            vec![
                ("manager".to_owned(), "ok".to_owned()),
                ("worker-1".to_owned(), "unavailable".to_owned()),
                ("worker-2".to_owned(), "unavailable".to_owned()),
            ],
            "the offline member is reported without being dialed"
        );
        assert!(body["snapshotAt"].is_string());

        let (status, body) =
            send(&fixture, "GET", "/api/v1/containers?node=manager", None, "").await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["nodes"].as_array().unwrap().len(), 1);
        let (status, _) = send(&fixture, "GET", "/api/v1/containers?node=ghost", None, "").await?;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = send(&fixture, "GET", "/api/v1/containers?node=a/b", None, "").await?;
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
async fn postgres_list_drains_pages_and_refuses_past_the_cap() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection, 2_001).await?;
        let (status, body) = send(&fixture, "GET", "/api/v1/containers", None, "").await?;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(body["error"], "too_many_containers");
        assert!(body["hint"].as_str().unwrap().contains("?node="));
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);

    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection, 1_200).await?;
        let (status, body) = send(&fixture, "GET", "/api/v1/containers", None, "").await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body["items"].as_array().unwrap().len(),
            1_200,
            "three pages drained"
        );
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_actions_are_admitted_and_run_against_the_node_agent() -> Result<(), Box<dyn Error>>
{
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection, 1).await?;
        let principal = fixture.principal;
        let id = "a".repeat(64);
        let path = |action: &str| format!("/api/v1/containers/manager/{id}/actions/{action}");

        let (status, _) = send(&fixture, "POST", &path("restart"), None, "").await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "a key is required");
        for (bad_path, expected) in [
            (path("explode"), StatusCode::BAD_REQUEST),
            (
                "/api/v1/containers/manager/short/actions/stop".to_owned(),
                StatusCode::BAD_REQUEST,
            ),
            (
                format!("/api/v1/containers/ghost/{id}/actions/stop"),
                StatusCode::NOT_FOUND,
            ),
        ] {
            let (status, _) = send(
                &fixture,
                "POST",
                &bad_path,
                Some("018f2e34-0000-7000-8000-00000000000a"),
                "",
            )
            .await?;
            assert_eq!(status, expected, "{bad_path}");
        }

        // Restart succeeds; plain remove of a running container fails; forced remove succeeds.
        let mut outcomes = Vec::new();
        for (index, (action, body)) in [
            ("restart", r#"{"timeoutSeconds":5}"#),
            ("remove", ""),
            ("remove", r#"{"force":true}"#),
        ]
        .into_iter()
        .enumerate()
        {
            let key = format!("018f2e34-0000-7000-8000-00000000010{index}");
            let (status, admitted) =
                send(&fixture, "POST", &path(action), Some(&key), body).await?;
            assert_eq!(status, StatusCode::ACCEPTED);
            let snapshot = run_until_terminal(
                &fixture,
                &principal,
                admitted["operationId"].as_str().unwrap(),
            )
            .await?;
            outcomes.push((
                snapshot.operation.kind,
                snapshot.operation.state,
                snapshot.last_error_code,
            ));
        }
        assert_eq!(
            outcomes,
            vec![
                (
                    "container.restart".to_owned(),
                    OperationState::Succeeded,
                    None
                ),
                (
                    "container.remove".to_owned(),
                    OperationState::Failed,
                    Some("container_running".to_owned())
                ),
                (
                    "container.remove".to_owned(),
                    OperationState::Succeeded,
                    None
                ),
            ]
        );
        let actions = fixture.actions.lock().unwrap().clone();
        assert_eq!(
            actions
                .iter()
                .map(|(_, _, action, force)| (*action, *force))
                .collect::<Vec<_>>(),
            vec![
                (AgentContainerAction::Restart, false),
                (AgentContainerAction::Remove, false),
                (AgentContainerAction::Remove, true),
            ],
            "force reaches the agent only when the request said so"
        );
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_bulk_reports_per_target_results_after_a_partial_failure()
-> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection, 1).await?;
        let principal = fixture.principal;
        let body = serde_json::json!({
            "action": "pause",
            "targets": [
                {"nodeId": "manager", "containerId": "a".repeat(64)},
                {"nodeId": "manager", "containerId": "b".repeat(64)},
                {"nodeId": "worker-1", "containerId": "c".repeat(64)},
            ],
        })
        .to_string();
        let (status, admitted) = send(&fixture, "POST", "/api/v1/containers/actions/bulk", Some("018f2e34-0000-7000-8000-000000000200"), &body).await?;
        assert_eq!(status, StatusCode::ACCEPTED);
        let snapshot = run_until_terminal(&fixture, &principal, admitted["operationId"].as_str().unwrap()).await?;
        assert_eq!(snapshot.operation.state, OperationState::Failed);
        assert_eq!(snapshot.last_error_code.as_deref(), Some("partial_failure"));
        let detail = snapshot.progress.unwrap();
        assert_eq!(detail["failed"], 2);
        let mut errors: Vec<_> = detail["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|result| (result["containerId"].as_str().unwrap()[..1].to_owned(), result["error"].as_str().map(ToOwned::to_owned)))
            .collect();
        errors.sort();
        assert_eq!(
            errors,
            vec![
                ("a".to_owned(), None),
                ("b".to_owned(), Some("engine_error".to_owned())),
                ("c".to_owned(), Some("agent_unavailable".to_owned())),
            ]
        );

        for bad in [
            serde_json::json!({"action": "pause", "targets": []}),
            serde_json::json!({"action": "explode", "targets": [{"nodeId": "manager", "containerId": "a".repeat(64)}]}),
            serde_json::json!({"action": "pause", "targets": [{"nodeId": "manager", "containerId": "a".repeat(64)}, {"nodeId": "manager", "containerId": "a".repeat(64)}]}),
        ] {
            let (status, _) = send(&fixture, "POST", "/api/v1/containers/actions/bulk", Some("018f2e34-0000-7000-8000-000000000201"), &bad.to_string()).await?;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
        }
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}
