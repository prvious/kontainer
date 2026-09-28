//! Volumes end to end against real PostgreSQL and fake agents.

use std::{
    collections::{BTreeMap, HashMap},
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
        executor::production_registry,
        worker::{Worker, WorkerSettings},
    },
    rpc::{
        AgentClient, AgentClientError, AgentContainerPage, AgentDescription, AgentVolume,
        AgentVolumeCreate, VolumeRemoval,
    },
    test_support::PostgresTestDatabase,
};

fn volume(name: &str, users: &[&str], size: Option<u64>) -> AgentVolume {
    AgentVolume {
        name: name.to_owned(),
        driver: "local".to_owned(),
        mountpoint: format!("/var/lib/docker/volumes/{name}/_data"),
        created_at_seconds: Some(1_790_000_000),
        labels: BTreeMap::from([(
            "com.docker.stack.namespace".to_owned(),
            "toolbox".to_owned(),
        )]),
        used_by: u32::try_from(users.len()).unwrap(),
        users: users.iter().map(|user| (*user).to_owned()).collect(),
        size_bytes: size,
    }
}

#[derive(Debug)]
struct FakeAgent {
    node_id: String,
    volumes: Mutex<Vec<AgentVolume>>,
    created: Mutex<Vec<AgentVolumeCreate>>,
}

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
        Err(AgentClientError::Unavailable)
    }

    async fn list_volumes(
        &self,
        _: &CancellationToken,
    ) -> Result<Vec<AgentVolume>, AgentClientError> {
        Ok(self.volumes.lock().unwrap().clone())
    }

    async fn create_volume(
        &self,
        request: &AgentVolumeCreate,
        _: &CancellationToken,
    ) -> Result<AgentVolume, AgentClientError> {
        let mut volumes = self.volumes.lock().unwrap();
        if volumes.iter().any(|volume| volume.name == request.name) {
            return Err(AgentClientError::VolumeExists);
        }
        self.created.lock().unwrap().push(request.clone());
        let created = AgentVolume {
            labels: request.labels.clone(),
            ..volume(&request.name, &[], None)
        };
        volumes.push(created.clone());
        Ok(created)
    }

    async fn remove_volume(
        &self,
        name: &str,
        _: &CancellationToken,
    ) -> Result<VolumeRemoval, AgentClientError> {
        let mut volumes = self.volumes.lock().unwrap();
        let Some(index) = volumes.iter().position(|volume| volume.name == name) else {
            return Err(AgentClientError::NotFound);
        };
        if !volumes[index].users.is_empty() {
            return Ok(VolumeRemoval::InUse(volumes[index].users.clone()));
        }
        volumes.remove(index);
        Ok(VolumeRemoval::Removed)
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

struct Fixture {
    router: Router,
    context: AppContext,
    directory: Arc<FakeDirectory>,
    authorization: String,
    principal: PrincipalContext,
}

async fn fixture(database: &sea_orm::DatabaseConnection) -> Result<Fixture, Box<dyn Error>> {
    for node_id in ["manager", "worker-1", "worker-2"] {
        record_check_in::execute(database, "test", check_in(node_id, None)).await?;
    }
    let agent = |node_id: &str, volumes| {
        (
            node_id.to_owned(),
            Arc::new(FakeAgent {
                node_id: node_id.to_owned(),
                volumes: Mutex::new(volumes),
                created: Mutex::new(Vec::new()),
            }),
        )
    };
    let directory = Arc::new(FakeDirectory(HashMap::from([
        agent(
            "manager",
            vec![volume(
                "toolbox_redis-data",
                &["toolbox_redis.1.x"],
                Some(4096),
            )],
        ),
        agent("worker-1", vec![volume("scratch", &[], None)]),
        // The same name on another node is another row.
        agent("worker-2", vec![volume("scratch", &[], Some(0))]),
    ])));
    let config = Config::from_folder(&Environment::Test, Path::new("config"))?;
    let context = App::after_context(
        AppContext::builder(Environment::Test, database.clone(), config).build(),
    )
    .await?;
    context.shared_store.insert(SharedAgentDirectory(
        directory.clone() as Arc<dyn AgentDirectory>
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
        directory,
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
    let bytes = to_bytes(response.into_body(), 1 << 20).await?;
    Ok((
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    ))
}

async fn run(fixture: &Fixture, operation_id: &str) -> Result<OperationSnapshot, Box<dyn Error>> {
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
    let mut snapshot = get::execute(&fixture.context.db, &fixture.principal, id).await?;
    for _ in 0..300 {
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
async fn postgres_volumes_list_per_node_and_answer_the_conflict_query() -> Result<(), Box<dyn Error>>
{
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection).await?;
        let (status, body) = send(&fixture, "GET", "/api/v1/volumes", None, "").await?;
        assert_eq!(status, StatusCode::OK);
        let rows: Vec<_> = body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                (
                    item["nodeId"].as_str().unwrap(),
                    item["name"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            vec![
                ("manager", "toolbox_redis-data"),
                ("worker-1", "scratch"),
                ("worker-2", "scratch"),
            ]
        );
        let redis = &body["items"][0];
        assert_eq!(redis["usedBy"], 1);
        assert_eq!(redis["users"], serde_json::json!(["toolbox_redis.1.x"]));
        assert_eq!(redis["sizeBytes"], 4096);
        assert_eq!(
            redis["mountPoint"],
            "/var/lib/docker/volumes/toolbox_redis-data/_data"
        );
        assert_eq!(redis["createdAt"], "2026-09-21T14:13:20.000Z");
        assert!(
            body["items"][1]["sizeBytes"].is_null(),
            "unknown until the node's df"
        );

        // The dialog's conflict check: one node, one name.
        let (_, taken) = send(
            &fixture,
            "GET",
            "/api/v1/volumes?node=worker-1&name=scratch",
            None,
            "",
        )
        .await?;
        assert_eq!(taken["items"].as_array().unwrap().len(), 1);
        assert_eq!(taken["nodes"].as_array().unwrap().len(), 1);
        let (_, free) = send(
            &fixture,
            "GET",
            "/api/v1/volumes?node=manager&name=scratch",
            None,
            "",
        )
        .await?;
        assert!(free["items"].as_array().unwrap().is_empty());
        let (status, _) = send(&fixture, "GET", "/api/v1/volumes?node=ghost", None, "").await?;
        assert_eq!(status, StatusCode::NOT_FOUND);
        for (query, field) in [
            ("name=a", "name"),
            ("node=..", "node"),
            ("sort=size", "sort"),
        ] {
            let (status, error) = send(
                &fixture,
                "GET",
                &format!("/api/v1/volumes?{query}"),
                None,
                "",
            )
            .await?;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{query}");
            assert!(error["fields"][field].is_string(), "{query}");
        }
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_volume_creation_validates_and_reports_its_result() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection).await?;
        for (name, driver, labels, field) in [
            ("a", "local", serde_json::json!({}), "name"),
            ("-data", "local", serde_json::json!({}), "name"),
            ("has space", "local", serde_json::json!({}), "name"),
            (&"a".repeat(256) as &str, "local", serde_json::json!({}), "name"),
            ("pg-data", "nfs", serde_json::json!({}), "driver"),
            ("pg-data", "local", serde_json::json!({"a=b": "x"}), "labels"),
        ] {
            let body = serde_json::json!({ "nodeId": "worker-1", "name": name, "driver": driver, "labels": labels }).to_string();
            let (status, error) = send(&fixture, "POST", "/api/v1/volumes", Some("018f2e34-0000-7000-8000-000000000401"), &body).await?;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{name}");
            assert!(error["fields"][field].is_string(), "{name}: {error}");
        }
        let ghost = serde_json::json!({ "nodeId": "ghost", "name": "pg-data", "driver": "local" }).to_string();
        let (status, error) = send(&fixture, "POST", "/api/v1/volumes", Some("018f2e34-0000-7000-8000-000000000402"), &ghost).await?;
        assert_eq!((status, error["fields"]["nodeId"].as_str()), (StatusCode::BAD_REQUEST, Some("invalid")));

        let body = serde_json::json!({ "nodeId": "worker-1", "name": "pg-data", "driver": "local", "labels": {"app": "billing"} }).to_string();
        let (status, admitted) = send(&fixture, "POST", "/api/v1/volumes", Some("018f2e34-0000-7000-8000-000000000403"), &body).await?;
        assert_eq!(status, StatusCode::ACCEPTED);
        let created = run(&fixture, admitted["operationId"].as_str().unwrap()).await?;
        assert_eq!(created.operation.state, OperationState::Succeeded);
        assert_eq!(created.progress.unwrap()["name"], "pg-data");
        let worker = &fixture.directory.0["worker-1"];
        assert_eq!(worker.created.lock().unwrap()[0].labels.get("app").map(String::as_str), Some("billing"));

        let (_, again) = send(&fixture, "POST", "/api/v1/volumes", Some("018f2e34-0000-7000-8000-000000000404"), &body).await?;
        let exists = run(&fixture, again["operationId"].as_str().unwrap()).await?;
        assert_eq!(exists.operation.state, OperationState::Failed);
        assert_eq!(exists.last_error_code.as_deref(), Some("volume_exists"));
        assert_eq!(
            exists.error_message.as_deref(),
            Some("A volume named pg-data already exists on worker-1. Pick another name or another node.")
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
async fn postgres_volume_removal_refuses_mounted_volumes_with_their_users()
-> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection).await?;
        let (status, admitted) = send(
            &fixture,
            "DELETE",
            "/api/v1/volumes/manager/toolbox_redis-data",
            Some("018f2e34-0000-7000-8000-000000000410"),
            "",
        )
        .await?;
        assert_eq!(status, StatusCode::ACCEPTED);
        let in_use = run(&fixture, admitted["operationId"].as_str().unwrap()).await?;
        assert_eq!(in_use.operation.state, OperationState::Failed);
        assert_eq!(in_use.last_error_code.as_deref(), Some("volume_in_use"));
        assert!(
            in_use
                .error_message
                .as_deref()
                .unwrap()
                .starts_with("In use by toolbox_redis.1.x.")
        );
        assert_eq!(
            in_use.progress.unwrap()["inUseBy"],
            serde_json::json!(["toolbox_redis.1.x"])
        );

        let (_, admitted) = send(
            &fixture,
            "DELETE",
            "/api/v1/volumes/worker-2/scratch",
            Some("018f2e34-0000-7000-8000-000000000411"),
            "",
        )
        .await?;
        let removed = run(&fixture, admitted["operationId"].as_str().unwrap()).await?;
        assert_eq!(removed.operation.state, OperationState::Succeeded);
        assert!(
            fixture.directory.0["worker-2"]
                .volumes
                .lock()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            fixture.directory.0["worker-1"]
                .volumes
                .lock()
                .unwrap()
                .len(),
            1,
            "only that node"
        );

        let (_, admitted) = send(
            &fixture,
            "DELETE",
            "/api/v1/volumes/worker-2/scratch",
            Some("018f2e34-0000-7000-8000-000000000412"),
            "",
        )
        .await?;
        let missing = run(&fixture, admitted["operationId"].as_str().unwrap()).await?;
        assert_eq!(missing.last_error_code.as_deref(), Some("not_found"));
        let (status, _) = send(
            &fixture,
            "DELETE",
            "/api/v1/volumes/worker-2/a",
            Some("018f2e34-0000-7000-8000-000000000413"),
            "",
        )
        .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}
