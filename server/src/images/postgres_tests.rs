//! Images end to end against real PostgreSQL and fake agents.

use std::{collections::HashMap, error::Error, path::Path, sync::Arc, time::Duration};

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
        AgentClient, AgentClientError, AgentContainerPage, AgentDescription, AgentImage,
        AgentPullEvent, AgentPullStream,
    },
    test_support::PostgresTestDatabase,
};

fn image_id(fill: char) -> String {
    format!("sha256:{}", fill.to_string().repeat(64))
}

#[derive(Debug)]
struct FakeAgent {
    node_id: String,
    images: Vec<AgentImage>,
    pull_fails: bool,
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

    async fn list_images(
        &self,
        _: &CancellationToken,
    ) -> Result<Vec<AgentImage>, AgentClientError> {
        Ok(self.images.clone())
    }

    async fn remove_image(
        &self,
        image_id: &str,
        force: bool,
        _: &CancellationToken,
    ) -> Result<(), AgentClientError> {
        let Some(image) = self.images.iter().find(|image| image.image_id == image_id) else {
            return Err(AgentClientError::NotFound);
        };
        if image.used_by > 0 && !force {
            return Err(AgentClientError::ImageInUse);
        }
        Ok(())
    }

    async fn pull_image(
        &self,
        _reference: &str,
        _: &CancellationToken,
    ) -> Result<AgentPullStream, AgentClientError> {
        let progress = |layer: &str, status: &str, current, total| AgentPullEvent::Progress {
            layer_id: Some(layer.to_owned()),
            status: status.to_owned(),
            current,
            total,
        };
        let mut events = vec![
            progress("l1", "Pulling fs layer", None, None),
            progress("l2", "Pulling fs layer", None, None),
            progress("l1", "Downloading", Some(100), Some(400)),
        ];
        if self.pull_fails {
            events.push(AgentPullEvent::Failed(
                "unauthorized: authentication required".to_owned(),
            ));
        } else {
            events.extend([
                progress("l1", "Pull complete", None, None),
                progress("l2", "Already exists", None, None),
            ]);
        }
        Ok(Box::pin(futures_util::stream::iter(events)))
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

fn image(fill: char, tags: &[&str], used_by: u32) -> AgentImage {
    AgentImage {
        image_id: image_id(fill),
        repo_tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        repo_digests: Vec::new(),
        size_bytes: 42_000_000,
        created_at_seconds: 1_700_000_000,
        used_by,
        dangling: tags.is_empty(),
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
    let agent = |node_id: &str, images, pull_fails| {
        (
            node_id.to_owned(),
            Arc::new(FakeAgent {
                node_id: node_id.to_owned(),
                images,
                pull_fails,
            }),
        )
    };
    let directory = Arc::new(FakeDirectory(HashMap::from([
        agent(
            "manager",
            vec![image('a', &["nginx:1.27"], 2), image('c', &[], 0)],
            false,
        ),
        agent(
            "worker-1",
            vec![image('a', &["nginx:1.27", "nginx:latest"], 1)],
            false,
        ),
        agent("worker-2", vec![image('b', &["alpine:3.20"], 0)], true),
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
        manager: None,
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
async fn postgres_images_merge_by_id_and_validate_pulls() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection).await?;
        let (status, body) = send(&fixture, "GET", "/api/v1/images", None, "").await?;
        assert_eq!(status, StatusCode::OK);
        let items = body["items"].as_array().unwrap();
        assert_eq!(items.len(), 3);
        let nginx = items
            .iter()
            .find(|item| item["imageId"] == image_id('a'))
            .unwrap();
        assert_eq!(nginx["nodeIds"], serde_json::json!(["manager", "worker-1"]));
        assert_eq!(nginx["usedBy"], 3);
        assert_eq!(nginx["sizeBytes"], 42_000_000);
        assert_eq!(
            nginx["repoTags"],
            serde_json::json!(["nginx:1.27", "nginx:latest"])
        );
        assert_eq!(body["nodes"].as_array().unwrap().len(), 3);

        for (reference, problem) in [
            ("Alpine", "invalid_name"),
            ("alpine:", "invalid_tag"),
            ("a..b/c", "invalid_registry"),
            ("", "empty_or_too_long"),
        ] {
            let body =
                serde_json::json!({ "reference": reference, "nodeIds": ["manager"] }).to_string();
            let (status, error) = send(
                &fixture,
                "POST",
                "/api/v1/images/pulls",
                Some("018f2e34-0000-7000-8000-000000000301"),
                &body,
            )
            .await?;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{reference}");
            assert_eq!(error["fields"]["reference"], problem, "{reference}");
        }
        let unknown =
            serde_json::json!({ "reference": "alpine:3.20", "nodeIds": ["ghost"] }).to_string();
        let (status, error) = send(
            &fixture,
            "POST",
            "/api/v1/images/pulls",
            Some("018f2e34-0000-7000-8000-000000000302"),
            &unknown,
        )
        .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error["fields"]["nodeIds"], "invalid");
        let (status, _) = send(
            &fixture,
            "DELETE",
            "/api/v1/images/nginx",
            Some("018f2e34-0000-7000-8000-000000000303"),
            r#"{"nodeIds":"all"}"#,
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

#[tokio::test]
async fn postgres_pull_keeps_going_when_one_node_fails() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection).await?;
        let body = serde_json::json!({ "reference": "alpine", "nodeIds": ["manager", "worker-1", "worker-2"] }).to_string();
        let (status, admitted) = send(&fixture, "POST", "/api/v1/images/pulls", Some("018f2e34-0000-7000-8000-000000000310"), &body).await?;
        assert_eq!(status, StatusCode::ACCEPTED);
        let snapshot = run(&fixture, admitted["operationId"].as_str().unwrap()).await?;
        assert_eq!(snapshot.operation.payload["reference"], "alpine:latest", "normalized at admission");
        assert_eq!(snapshot.operation.state, OperationState::Failed);
        assert_eq!(snapshot.last_error_code.as_deref(), Some("partial_failure"));
        let nodes = snapshot.progress.unwrap()["nodes"].as_array().unwrap().clone();
        let summary: Vec<_> = nodes
            .iter()
            .map(|node| (node["nodeId"].as_str().unwrap().to_owned(), node["status"].as_str().unwrap().to_owned()))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("manager".to_owned(), "done".to_owned()),
                ("worker-1".to_owned(), "done".to_owned()),
                ("worker-2".to_owned(), "failed".to_owned()),
            ]
        );
        let manager = &nodes[0];
        assert_eq!((manager["layersDone"].clone(), manager["layersTotal"].clone()), (2.into(), 2.into()));
        assert_eq!(nodes[2]["error"], "unauthorized: authentication required", "registry error verbatim");
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_remove_reports_per_node_results() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection).await?;
        let path = format!("/api/v1/images/{}", image_id('a'));
        // In use on both nodes: without force it fails everywhere with a readable code.
        let (status, admitted) = send(
            &fixture,
            "DELETE",
            &path,
            Some("018f2e34-0000-7000-8000-000000000320"),
            r#"{"nodeIds":"all"}"#,
        )
        .await?;
        assert_eq!(status, StatusCode::ACCEPTED);
        let in_use = run(&fixture, admitted["operationId"].as_str().unwrap()).await?;
        assert_eq!(in_use.operation.state, OperationState::Failed);
        assert_eq!(in_use.last_error_code.as_deref(), Some("image_in_use"));
        assert!(in_use.error_message.as_deref().unwrap().contains("force"));
        let nodes = in_use.progress.unwrap()["nodes"]
            .as_array()
            .unwrap()
            .clone();
        let skipped: Vec<_> = nodes
            .iter()
            .filter(|node| node["skipped"] == true)
            .map(|node| node["nodeId"].clone())
            .collect();
        assert_eq!(
            skipped,
            vec![serde_json::json!("worker-2")],
            "a member without it is skipped"
        );

        // Forced from one node only.
        let (_, admitted) = send(
            &fixture,
            "DELETE",
            &path,
            Some("018f2e34-0000-7000-8000-000000000321"),
            r#"{"nodeIds":["worker-1"],"force":true}"#,
        )
        .await?;
        let forced = run(&fixture, admitted["operationId"].as_str().unwrap()).await?;
        assert_eq!(forced.operation.state, OperationState::Succeeded);
        let nodes = forced.progress.unwrap()["nodes"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(nodes.len(), 1);
        assert_eq!(
            nodes[0],
            serde_json::json!({"nodeId": "worker-1", "ok": true})
        );
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}
