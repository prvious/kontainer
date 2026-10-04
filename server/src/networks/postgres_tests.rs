//! Networks end to end against real PostgreSQL, a fake manager engine, and fake agents.

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
use kontainer_engine::{NetworkCreate, NetworkDetail, NetworkSummary};
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
        AgentClient, AgentClientError, AgentContainerPage, AgentDescription, AgentNetwork,
        AgentNetworkCreate, AgentNetworkDetail, AgentNetworkEndpoint, NetworkRemoval,
    },
    swarm::engine::{ManagerEngine, ManagerError, SharedManagerEngine},
    test_support::PostgresTestDatabase,
};

fn id(fill: char) -> String {
    fill.to_string().repeat(64)
}

fn agent_network(fill: char, name: &str, scope: &str, containers: u32) -> AgentNetwork {
    AgentNetwork {
        id: id(fill),
        name: name.to_owned(),
        driver: if scope == "swarm" {
            "overlay"
        } else {
            "bridge"
        }
        .to_owned(),
        scope: scope.to_owned(),
        subnets: vec!["10.0.1.0/24".to_owned()],
        containers,
        builtin: kontainer_engine::is_builtin_network(name),
        ..AgentNetwork::default()
    }
}

fn swarm_network(fill: char, name: &str) -> NetworkSummary {
    NetworkSummary {
        id: id(fill),
        name: name.to_owned(),
        driver: "overlay".to_owned(),
        scope: "swarm".to_owned(),
        attachable: true,
        ingress: name == "ingress",
        created_at_seconds: Some(1_790_000_000),
        ..NetworkSummary::default()
    }
}

#[derive(Debug, Default)]
struct FakeManager {
    networks: Mutex<Vec<NetworkSummary>>,
    created: Mutex<Vec<NetworkCreate>>,
    removed: Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl ManagerEngine for FakeManager {
    async fn list_networks(
        &self,
        _: &CancellationToken,
    ) -> Result<Vec<NetworkSummary>, ManagerError> {
        Ok(self.networks.lock().unwrap().clone())
    }

    async fn inspect_network(
        &self,
        network: &str,
        _: &CancellationToken,
    ) -> Result<NetworkDetail, ManagerError> {
        self.networks
            .lock()
            .unwrap()
            .iter()
            .find(|candidate| candidate.id == network)
            .cloned()
            .map(|network| NetworkDetail {
                network,
                endpoints: Vec::new(),
            })
            .ok_or(ManagerError::NotFound)
    }

    async fn create_network(
        &self,
        network: &NetworkCreate,
        _: &CancellationToken,
    ) -> Result<String, ManagerError> {
        self.created.lock().unwrap().push(network.clone());
        self.networks
            .lock()
            .unwrap()
            .push(swarm_network('n', &network.name));
        Ok(id('n'))
    }

    async fn remove_network(
        &self,
        network: &str,
        _: &CancellationToken,
    ) -> Result<(), ManagerError> {
        self.removed.lock().unwrap().push(network.to_owned());
        self.networks
            .lock()
            .unwrap()
            .retain(|candidate| candidate.id != network);
        Ok(())
    }
}

/// The manager engine of a controller on a worker: every call says it isn't a manager.
#[derive(Debug)]
struct WorkerHost;

#[async_trait::async_trait]
impl ManagerEngine for WorkerHost {
    async fn list_networks(
        &self,
        _: &CancellationToken,
    ) -> Result<Vec<NetworkSummary>, ManagerError> {
        Err(ManagerError::NotAManager)
    }

    async fn inspect_network(
        &self,
        _: &str,
        _: &CancellationToken,
    ) -> Result<NetworkDetail, ManagerError> {
        Err(ManagerError::NotAManager)
    }

    async fn create_network(
        &self,
        _: &NetworkCreate,
        _: &CancellationToken,
    ) -> Result<String, ManagerError> {
        Err(ManagerError::NotAManager)
    }

    async fn remove_network(&self, _: &str, _: &CancellationToken) -> Result<(), ManagerError> {
        Err(ManagerError::NotAManager)
    }
}

#[derive(Debug)]
struct FakeAgent {
    node_id: String,
    networks: Vec<AgentNetwork>,
    endpoints: HashMap<String, Vec<&'static str>>,
    created: Mutex<Vec<AgentNetworkCreate>>,
    removed: Mutex<Vec<String>>,
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

    async fn list_networks(
        &self,
        _: &CancellationToken,
    ) -> Result<Vec<AgentNetwork>, AgentClientError> {
        Ok(self.networks.clone())
    }

    async fn inspect_network(
        &self,
        network: &str,
        _: &CancellationToken,
    ) -> Result<AgentNetworkDetail, AgentClientError> {
        let found = self
            .networks
            .iter()
            .find(|candidate| candidate.id == network)
            .cloned()
            .ok_or(AgentClientError::NotFound)?;
        let endpoints = self
            .endpoints
            .get(network)
            .map(|names| {
                names
                    .iter()
                    .enumerate()
                    .map(|(index, name)| AgentNetworkEndpoint {
                        container_id: format!("{index:064x}"),
                        name: (*name).to_owned(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(AgentNetworkDetail {
            network: found,
            endpoints,
        })
    }

    async fn create_network(
        &self,
        network: &AgentNetworkCreate,
        _: &CancellationToken,
    ) -> Result<String, AgentClientError> {
        // Another request created it after admission: Docker's 409 reaches the controller as
        // `container_running`.
        if network.name == "raced-net" {
            return Err(AgentClientError::ContainerRunning);
        }
        self.created.lock().unwrap().push(network.clone());
        Ok(id('m'))
    }

    async fn remove_network(
        &self,
        network: &str,
        _: &CancellationToken,
    ) -> Result<NetworkRemoval, AgentClientError> {
        if let Some(names) = self
            .endpoints
            .get(network)
            .filter(|names| !names.is_empty())
        {
            return Ok(NetworkRemoval::InUse(
                names.iter().map(|name| (*name).to_owned()).collect(),
            ));
        }
        self.removed.lock().unwrap().push(network.to_owned());
        Ok(NetworkRemoval::Removed)
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
    manager: Option<Arc<FakeManager>>,
    authorization: String,
    principal: PrincipalContext,
}

async fn fixture(
    database: &sea_orm::DatabaseConnection,
    with_manager: bool,
) -> Result<Fixture, Box<dyn Error>> {
    for node_id in ["manager", "worker-1", "worker-2"] {
        record_check_in::execute(database, "test", check_in(node_id, None)).await?;
    }
    let agent = |node_id: &str,
                 networks: Vec<AgentNetwork>,
                 endpoints: HashMap<String, Vec<&'static str>>| {
        (
            node_id.to_owned(),
            Arc::new(FakeAgent {
                node_id: node_id.to_owned(),
                networks,
                endpoints,
                created: Mutex::new(Vec::new()),
                removed: Mutex::new(Vec::new()),
            }),
        )
    };
    let directory = Arc::new(FakeDirectory(HashMap::from([
        agent(
            "manager",
            vec![
                agent_network('b', "bridge", "local", 0),
                agent_network('s', "toolbox_kontainer", "swarm", 2),
            ],
            HashMap::from([(id('s'), vec!["toolbox_nginx.1", "toolbox_nginx.2"])]),
        ),
        agent(
            "worker-1",
            vec![
                agent_network('c', "bridge", "local", 0),
                agent_network('s', "toolbox_kontainer", "swarm", 1),
            ],
            HashMap::from([(id('s'), vec!["toolbox_redis.1"])]),
        ),
        agent(
            "worker-2",
            vec![
                agent_network('l', "scratch-net", "local", 0),
                agent_network('i', "ingress", "swarm", 0),
            ],
            HashMap::new(),
        ),
    ])));
    let manager = with_manager.then(|| {
        Arc::new(FakeManager {
            networks: Mutex::new(vec![
                swarm_network('s', "toolbox_kontainer"),
                swarm_network('i', "ingress"),
                swarm_network('e', "empty-overlay"),
            ]),
            ..FakeManager::default()
        })
    });
    let config = Config::from_folder(&Environment::Test, Path::new("config"))?;
    let context = App::after_context(
        AppContext::builder(Environment::Test, database.clone(), config).build(),
    )
    .await?;
    context.shared_store.insert(SharedAgentDirectory(
        directory.clone() as Arc<dyn AgentDirectory>
    ));
    if let Some(manager) = &manager {
        context.shared_store.insert(SharedManagerEngine(
            manager.clone() as Arc<dyn ManagerEngine>
        ));
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
        directory,
        manager,
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
        manager: fixture
            .manager
            .clone()
            .map(|manager| manager as Arc<dyn ManagerEngine>),
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
async fn postgres_networks_merge_swarm_and_local_lists_and_name_attached_containers()
-> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection, true).await?;
        let (status, body) = send(&fixture, "GET", "/api/v1/networks", None, "").await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["manager"], "ok");
        let rows: Vec<_> = body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                (
                    item["name"].as_str().unwrap().to_owned(),
                    item["scope"].as_str().unwrap().to_owned(),
                    item["containers"].as_u64().unwrap(),
                    item["nodeIds"].as_array().unwrap().len(),
                    item["builtin"].as_bool().unwrap(),
                )
            })
            .collect();
        let row = |name: &str, scope: &str, containers: u64, nodes: usize, builtin: bool| {
            (
                name.to_owned(),
                scope.to_owned(),
                containers,
                nodes,
                builtin,
            )
        };
        assert_eq!(
            rows,
            vec![
                row("bridge", "local", 0, 1, true),
                row("bridge", "local", 0, 1, true),
                row("empty-overlay", "swarm", 0, 0, false),
                row("ingress", "swarm", 0, 0, true),
                row("scratch-net", "local", 0, 1, false),
                row("toolbox_kontainer", "swarm", 3, 2, false),
            ]
        );

        let (_, attached) = send(
            &fixture,
            "GET",
            &format!("/api/v1/networks/{}", id('s')),
            None,
            "",
        )
        .await?;
        let names: Vec<_> = attached["containers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            vec!["toolbox_nginx.1", "toolbox_nginx.2", "toolbox_redis.1"]
        );
        assert_eq!(attached["containers"][2]["nodeId"], "worker-1");
        let (status, _) = send(
            &fixture,
            "GET",
            &format!("/api/v1/networks/{}?node=worker-1", id('l')),
            None,
            "",
        )
        .await?;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "worker-1 doesn't have scratch-net"
        );
        let (status, _) = send(&fixture, "GET", "/api/v1/networks/nope", None, "").await?;
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
async fn postgres_network_creation_validates_and_runs_on_the_manager_or_the_node()
-> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection, true).await?;
        for (body, field, problem) in [
            (serde_json::json!({"name": "-bad", "driver": "overlay"}), "name", "invalid"),
            (serde_json::json!({"name": "ingress", "driver": "overlay"}), "name", "reserved"),
            (serde_json::json!({"name": "toolbox_kontainer", "driver": "overlay"}), "name", "taken"),
            (serde_json::json!({"name": "app", "driver": "macvlan"}), "driver", "unsupported"),
            (serde_json::json!({"name": "app", "driver": "bridge"}), "nodeId", "required"),
            (serde_json::json!({"name": "app", "driver": "overlay", "nodeId": "manager"}), "nodeId", "not_allowed"),
            (serde_json::json!({"name": "app", "driver": "bridge", "nodeId": "ghost"}), "nodeId", "invalid"),
            (serde_json::json!({"name": "app", "driver": "overlay", "subnet": "10.0.9.1/24"}), "subnet", "invalid"),
        ] {
            let (status, error) = send(&fixture, "POST", "/api/v1/networks", Some("018f2e34-0000-7000-8000-000000000501"), &body.to_string()).await?;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
            assert_eq!(error["fields"][field], problem, "{body}");
        }

        let overlay = serde_json::json!({"name": "app-network", "driver": "overlay", "attachable": true, "subnet": "10.0.9.0/24"}).to_string();
        let (status, admitted) = send(&fixture, "POST", "/api/v1/networks", Some("018f2e34-0000-7000-8000-000000000502"), &overlay).await?;
        assert_eq!(status, StatusCode::ACCEPTED);
        let created = run(&fixture, admitted["operationId"].as_str().unwrap()).await?;
        assert_eq!(created.operation.state, OperationState::Succeeded);
        let manager = fixture.manager.as_ref().unwrap();
        let made = manager.created.lock().unwrap()[0].clone();
        assert_eq!((made.driver.as_str(), made.attachable, made.subnet.as_deref()), ("overlay", true, Some("10.0.9.0/24")));
        // A retry whose response was lost replays, though the name is taken by now; a new key is
        // refused, and the same key with another body is a conflict.
        let (status, replay) = send(&fixture, "POST", "/api/v1/networks", Some("018f2e34-0000-7000-8000-000000000502"), &overlay).await?;
        assert_eq!((status, &replay["operationId"], &replay["replayed"]), (StatusCode::ACCEPTED, &admitted["operationId"], &serde_json::json!(true)));
        let (status, error) = send(&fixture, "POST", "/api/v1/networks", Some("018f2e34-0000-7000-8000-000000000504"), &overlay).await?;
        assert_eq!((status, error["fields"]["name"].as_str()), (StatusCode::BAD_REQUEST, Some("taken")));
        let other = serde_json::json!({"name": "app-network", "driver": "overlay"}).to_string();
        let (status, error) = send(&fixture, "POST", "/api/v1/networks", Some("018f2e34-0000-7000-8000-000000000502"), &other).await?;
        assert_eq!((status, error["error"].as_str()), (StatusCode::CONFLICT, Some("idempotency_conflict")));

        let bridge = serde_json::json!({"name": "local-net", "driver": "bridge", "nodeId": "worker-2", "attachable": true}).to_string();
        let (_, admitted) = send(&fixture, "POST", "/api/v1/networks", Some("018f2e34-0000-7000-8000-000000000503"), &bridge).await?;
        let created = run(&fixture, admitted["operationId"].as_str().unwrap()).await?;
        assert_eq!(created.operation.state, OperationState::Succeeded);
        assert_eq!(fixture.directory.0["worker-2"].created.lock().unwrap()[0].name, "local-net");
        assert_eq!(created.progress.unwrap()["nodeId"], "worker-2");

        // Created on the node after admission checked the name: Docker's 409 reads as such.
        let raced = serde_json::json!({"name": "raced-net", "driver": "bridge", "nodeId": "worker-2"}).to_string();
        let (_, admitted) = send(&fixture, "POST", "/api/v1/networks", Some("018f2e34-0000-7000-8000-000000000505"), &raced).await?;
        let failed = run(&fixture, admitted["operationId"].as_str().unwrap()).await?;
        assert_eq!(
            (failed.last_error_code.as_deref(), failed.error_message.as_deref()),
            (Some("name_taken"), Some("worker-2 already has a network named raced-net."))
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
async fn postgres_network_removal_refuses_builtins_and_attached_networks()
-> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection, true).await?;
        let (status, error) = send(&fixture, "DELETE", &format!("/api/v1/networks/{}", id('i')), Some("018f2e34-0000-7000-8000-000000000510"), "").await?;
        assert_eq!((status, error["error"].as_str()), (StatusCode::BAD_REQUEST, Some("builtin_network")));
        let (status, _) = send(&fixture, "DELETE", &format!("/api/v1/networks/{}?node=manager", id('b')), Some("018f2e34-0000-7000-8000-000000000511"), "").await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "local bridge is built in too");

        let (_, admitted) = send(&fixture, "DELETE", &format!("/api/v1/networks/{}", id('s')), Some("018f2e34-0000-7000-8000-000000000512"), "").await?;
        let in_use = run(&fixture, admitted["operationId"].as_str().unwrap()).await?;
        assert_eq!(in_use.operation.state, OperationState::Failed);
        assert_eq!(in_use.last_error_code.as_deref(), Some("in_use"));
        assert_eq!(
            in_use.error_message.as_deref(),
            Some("Can't remove toolbox_kontainer: 3 containers are attached (toolbox_nginx.1, toolbox_nginx.2, toolbox_redis.1). Remove or move them first.")
        );
        let manager = fixture.manager.as_ref().unwrap();
        assert!(manager.removed.lock().unwrap().is_empty(), "nothing reached the manager");

        let (_, admitted) = send(&fixture, "DELETE", &format!("/api/v1/networks/{}", id('e')), Some("018f2e34-0000-7000-8000-000000000513"), "").await?;
        assert_eq!(run(&fixture, admitted["operationId"].as_str().unwrap()).await?.operation.state, OperationState::Succeeded);
        assert_eq!(*manager.removed.lock().unwrap(), vec![id('e')]);
        let (status, replay) = send(&fixture, "DELETE", &format!("/api/v1/networks/{}", id('e')), Some("018f2e34-0000-7000-8000-000000000513"), "").await?;
        assert_eq!(
            (status, &replay["operationId"], &replay["replayed"]),
            (StatusCode::ACCEPTED, &admitted["operationId"], &serde_json::json!(true)),
            "a retry whose response was lost replays, though the network is gone"
        );

        let (_, admitted) = send(&fixture, "DELETE", &format!("/api/v1/networks/{}?node=worker-2", id('l')), Some("018f2e34-0000-7000-8000-000000000514"), "").await?;
        assert_eq!(run(&fixture, admitted["operationId"].as_str().unwrap()).await?.operation.state, OperationState::Succeeded);
        assert_eq!(*fixture.directory.0["worker-2"].removed.lock().unwrap(), vec![id('l')]);
        let (status, _) = send(&fixture, "DELETE", &format!("/api/v1/networks/{}?node=worker-1", id('l')), Some("018f2e34-0000-7000-8000-000000000515"), "").await?;
        assert_eq!(status, StatusCode::NOT_FOUND, "not on that node");
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_without_a_manager_swarm_changes_answer_not_a_manager()
-> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let fixture = fixture(&database.connection, false).await?;
        // No manager engine at all, then one on a host that says it isn't a manager.
        for worker_hosted in [false, true] {
            if worker_hosted {
                fixture.context.shared_store.insert(SharedManagerEngine(
                    Arc::new(WorkerHost) as Arc<dyn ManagerEngine>
                ));
            }
            let (_, body) = send(&fixture, "GET", "/api/v1/networks", None, "").await?;
            assert_eq!(body["manager"], "not_a_manager");
            let swarm: Vec<_> = body["items"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|item| item["scope"] == "swarm")
                .map(|item| item["name"].as_str().unwrap())
                .collect();
            assert_eq!(
                swarm,
                vec!["ingress", "toolbox_kontainer"],
                "swarm rows still come from the nodes"
            );
            let overlay =
                serde_json::json!({"name": "app-network", "driver": "overlay"}).to_string();
            let (status, error) = send(
                &fixture,
                "POST",
                "/api/v1/networks",
                Some("018f2e34-0000-7000-8000-000000000520"),
                &overlay,
            )
            .await?;
            assert_eq!(
                (status, error["error"].as_str()),
                (StatusCode::SERVICE_UNAVAILABLE, Some("not_a_manager")),
                "worker-hosted: {worker_hosted}"
            );
            let (status, error) = send(
                &fixture,
                "DELETE",
                &format!("/api/v1/networks/{}", id('s')),
                Some("018f2e34-0000-7000-8000-000000000521"),
                "",
            )
            .await?;
            assert_eq!(
                (status, error["error"].as_str()),
                (StatusCode::SERVICE_UNAVAILABLE, Some("not_a_manager")),
                "worker-hosted: {worker_hosted}"
            );
        }
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}
