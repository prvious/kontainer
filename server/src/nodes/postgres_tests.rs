//! Membership against real PostgreSQL: check-in upserts, samples, and derived status.

use std::{error::Error, sync::Arc};

use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
};

use super::{
    actions::{database_now, list_nodes, record_check_in},
    domain::{CheckIn, NodeStatus, SampleReading},
    entities::{node, node_sample},
};
use tokio_util::sync::CancellationToken;

use crate::{
    swarm::engine::{ManagerEngine, ManagerError, SharedManagerEngine},
    test_support::PostgresTestDatabase,
};

pub(crate) fn check_in(node_id: &str, sample: Option<SampleReading>) -> CheckIn {
    CheckIn {
        node_id: node_id.to_owned(),
        agent_version: "0.1.0".to_owned(),
        architecture: "aarch64".to_owned(),
        advertise_address: "127.0.0.1:50052".to_owned(),
        hostname: node_id.to_owned(),
        swarm_node_id: Some("tj42rq5nzwf8".to_owned()),
        docker_version: "29.8.0".to_owned(),
        os: "Alpine Linux".to_owned(),
        cpu_cores: 4,
        memory_total_bytes: 8 << 30,
        sample,
    }
}

pub(crate) fn reading(at: chrono::DateTime<Utc>, cpu: f32) -> SampleReading {
    SampleReading {
        sampled_at: at,
        cpu_percent: cpu,
        memory_used_bytes: 1 << 30,
        containers_running: 3,
        containers_paused: 0,
        containers_exited: 1,
        net_rx_bytes_per_sec: Some(100.0),
        net_tx_bytes_per_sec: None,
    }
}

#[tokio::test]
async fn postgres_check_in_upserts_the_node_and_records_its_samples() -> Result<(), Box<dyn Error>>
{
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let db = &database.connection;
        let now = database_now(db).await?;
        record_check_in::execute(
            db,
            "development",
            check_in("manager", Some(reading(now, 10.0))),
        )
        .await?;
        let first = node::Entity::find_by_id("manager").one(db).await?.unwrap();

        let mut update = check_in("manager", Some(reading(now + Duration::seconds(5), 20.0)));
        update.hostname = "manager-renamed".to_owned();
        update.swarm_node_id = Some(String::new());
        record_check_in::execute(db, "development", update).await?;
        let second = node::Entity::find_by_id("manager").one(db).await?.unwrap();

        assert_eq!(
            node::Entity::find().all(db).await?.len(),
            1,
            "upsert, not insert"
        );
        assert_eq!(second.hostname, "manager-renamed");
        assert_eq!(
            second.swarm_node_id, None,
            "empty Swarm id is stored as null"
        );
        assert!(second.last_check_in_at >= first.last_check_in_at);

        let samples = node_sample::Entity::find()
            .filter(node_sample::Column::NodeId.eq("manager"))
            .order_by_asc(node_sample::Column::SampledAt)
            .all(db)
            .await?;
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[1].cpu_percent, 20.0);

        // A retry of the same sample time keeps the first row.
        record_check_in::execute(
            db,
            "development",
            check_in("manager", Some(reading(now, 99.0))),
        )
        .await?;
        let retried = node_sample::Entity::find()
            .filter(node_sample::Column::SampledAt.eq(samples[0].sampled_at))
            .one(db)
            .await?
            .unwrap();
        assert_eq!(retried.cpu_percent, 10.0);

        // A sample from a skewed clock lands at the database time instead.
        record_check_in::execute(
            db,
            "development",
            check_in("manager", Some(reading(now + Duration::hours(1), 1.0))),
        )
        .await?;
        let latest = node_sample::Entity::find()
            .order_by_desc(node_sample::Column::SampledAt)
            .one(db)
            .await?
            .unwrap();
        assert!(latest.sampled_at < now + Duration::minutes(1));

        // Rejected claims write nothing.
        let mut bad = check_in("worker-1", None);
        bad.advertise_address = "nowhere".to_owned();
        assert!(
            record_check_in::execute(db, "development", bad)
                .await
                .is_err()
        );
        assert!(
            node::Entity::find_by_id("worker-1")
                .one(db)
                .await?
                .is_none()
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
async fn postgres_status_boundaries_follow_the_database_clock() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let db = &database.connection;
        for node_id in ["fresh", "edge", "late", "gone"] {
            record_check_in::execute(db, "development", check_in(node_id, None)).await?;
        }
        let now = database_now(db).await?;
        for (node_id, silent) in [("fresh", 14), ("edge", 16), ("late", 59), ("gone", 61)] {
            let row = node::Entity::find_by_id(node_id).one(db).await?.unwrap();
            let mut row: node::ActiveModel = row.into();
            row.last_check_in_at = Set(now - Duration::seconds(silent));
            row.update(db).await?;
        }
        record_check_in::execute(
            db,
            "development",
            check_in("fresh", Some(reading(now - Duration::seconds(10), 5.0))),
        )
        .await?;
        record_check_in::execute(
            db,
            "development",
            check_in("fresh", Some(reading(now, 7.5))),
        )
        .await?;

        let (nodes, snapshot) = list_nodes::execute(db).await?;
        assert!(snapshot >= now);
        let statuses: Vec<_> = nodes
            .iter()
            .map(|node| (node.id.as_str(), node.status))
            .collect();
        assert_eq!(
            statuses,
            vec![
                ("edge", NodeStatus::Degraded),
                ("fresh", NodeStatus::Online),
                ("gone", NodeStatus::Offline),
                ("late", NodeStatus::Degraded),
            ]
        );
        let fresh = nodes.iter().find(|node| node.id == "fresh").unwrap();
        assert_eq!(
            fresh.latest_sample.unwrap().cpu_percent,
            7.5,
            "the newest sample per node"
        );
        assert!(
            nodes
                .iter()
                .filter(|node| node.id != "fresh")
                .all(|node| node.latest_sample.is_none())
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
async fn postgres_list_nodes_reads_each_nodes_newest_sample() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let db = &database.connection;
        let now = database_now(db).await?;
        // Newer rows first, so the read cannot lean on insertion order.
        for (node_id, newest_cpu, older_cpu) in [("alpha", 11.0, 91.0), ("beta", 22.0, 92.0)] {
            record_check_in::execute(
                db,
                "development",
                check_in(node_id, Some(reading(now, newest_cpu))),
            )
            .await?;
            record_check_in::execute(
                db,
                "development",
                check_in(
                    node_id,
                    Some(reading(now - Duration::seconds(5), older_cpu)),
                ),
            )
            .await?;
        }
        assert_eq!(node_sample::Entity::find().all(db).await?.len(), 4);

        let (nodes, _) = list_nodes::execute(db).await?;
        let newest: Vec<_> = nodes
            .iter()
            .map(|node| {
                (
                    node.id.as_str(),
                    node.latest_sample.map(|sample| sample.cpu_percent),
                )
            })
            .collect();
        assert_eq!(newest, vec![("alpha", Some(11.0)), ("beta", Some(22.0))]);
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

/// A manager that lists fixed Swarm members, or fails the way a manager without quorum does.
struct FakeManager(std::sync::Mutex<Result<Vec<kontainer_engine::SwarmNode>, ManagerError>>);

#[async_trait::async_trait]
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

    async fn list_swarm_nodes(
        &self,
        _: &CancellationToken,
    ) -> Result<Vec<kontainer_engine::SwarmNode>, ManagerError> {
        self.0.lock().unwrap().clone()
    }
}

const MANAGER_SWARM_ID: &str = "managerswarm0000000000000";
const WORKER_SWARM_ID: &str = "workeroneswarm00000000000";
const LONE_SWARM_ID: &str = "workerthreeswarm000000000";

fn swarm_members() -> Vec<kontainer_engine::SwarmNode> {
    use kontainer_engine::{SwarmAvailability, SwarmNode, SwarmNodeState, SwarmRole};

    let member = |id: &str, hostname: &str, role| SwarmNode {
        id: id.to_owned(),
        hostname: hostname.to_owned(),
        role,
        manager_reachable: (role == SwarmRole::Manager).then_some(true),
        availability: Some(SwarmAvailability::Active),
        state: SwarmNodeState::Ready,
        address: Some("10.0.1.10".to_owned()),
        docker_version: Some("29.1.0".to_owned()),
        os: Some("linux".to_owned()),
        architecture: Some("aarch64".to_owned()),
        cpu_cores: Some(2),
        memory_total_bytes: Some(4 << 30),
        ..SwarmNode::default()
    };
    let mut leader = member(MANAGER_SWARM_ID, "manager", SwarmRole::Manager);
    leader.is_leader = true;
    let mut drained = member(WORKER_SWARM_ID, "worker-1", SwarmRole::Worker);
    drained.availability = Some(SwarmAvailability::Drain);
    vec![
        leader,
        drained,
        member(LONE_SWARM_ID, "worker-3", SwarmRole::Worker),
    ]
}

async fn authorized_json(
    router: &axum::Router,
    path: &str,
    authorization: &str,
) -> Result<(axum::http::StatusCode, serde_json::Value), Box<dyn Error>> {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, header},
    };
    use tower::ServiceExt;

    let response = router
        .clone()
        .oneshot(
            Request::get(path)
                .header(header::AUTHORIZATION, authorization)
                .body(Body::empty())?,
        )
        .await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1 << 20).await?;
    Ok((status, serde_json::from_slice(&bytes)?))
}

#[tokio::test]
async fn postgres_nodes_join_swarm_members_and_summarize_the_cluster() -> Result<(), Box<dyn Error>>
{
    use loco_rs::{
        app::{AppContext, Hooks},
        config::Config,
        environment::Environment,
    };

    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let db = &database.connection;
        let config = Config::from_folder(&Environment::Test, std::path::Path::new("config"))?;
        let context = crate::app::App::after_context(
            AppContext::builder(Environment::Test, db.clone(), config).build(),
        )
        .await?;
        let fake = Arc::new(FakeManager(std::sync::Mutex::new(Ok(swarm_members()))));
        context
            .shared_store
            .insert(SharedManagerEngine(fake.clone() as Arc<dyn ManagerEngine>));
        let router = crate::app::App::routes(&context)
            .to_router::<crate::app::App>(context.clone(), axum::Router::new())?;
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
        .insert(db)
        .await?;
        let session = crate::auth::actions::create_session::execute(db, principal).await?;
        let authorization = format!("Bearer {}", session.access_plaintext);

        let at = database_now(db).await?;
        for (node_id, swarm_id, cpu) in [
            ("manager", Some(MANAGER_SWARM_ID), 30.0),
            ("worker-1", Some(WORKER_SWARM_ID), 10.0),
            ("standalone", None, 50.0),
        ] {
            let mut check_in = check_in(node_id, Some(reading(at, cpu)));
            check_in.swarm_node_id = swarm_id.map(str::to_owned);
            record_check_in::execute(db, "development", check_in).await?;
        }
        let row = node::Entity::find_by_id("standalone")
            .one(db)
            .await?
            .unwrap();
        let mut row: node::ActiveModel = row.into();
        row.last_check_in_at = Set(at - Duration::seconds(120));
        row.update(db).await?;

        let (status, body) = authorized_json(&router, "/api/v1/nodes", &authorization).await?;
        assert_eq!(status, axum::http::StatusCode::OK);
        let rows: Vec<_> = body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| {
                (
                    node["id"].as_str().unwrap().to_owned(),
                    node["role"].as_str().unwrap().to_owned(),
                    node["agentStatus"].as_str().unwrap().to_owned(),
                    node["health"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        let row = |id: &str, role: &str, agent: &str, health: &str| {
            (
                id.to_owned(),
                role.to_owned(),
                agent.to_owned(),
                health.to_owned(),
            )
        };
        assert_eq!(
            rows,
            vec![
                row("manager", "manager", "online", "active"),
                row("worker-1", "worker", "online", "active"),
                row(LONE_SWARM_ID, "worker", "no-agent", "degraded"),
                row("standalone", "unknown", "offline", "down"),
            ]
        );
        let manager = &body["items"][0];
        assert_eq!(manager["isLeader"], true);
        assert_eq!(manager["address"], "10.0.1.10");
        assert_eq!(manager["os"], "Alpine Linux", "agent facts win");
        // Unread fields dropped by ADR 0039.
        for dropped in [
            "swarmStatus",
            "swarmNodeId",
            "agentStartedAt",
            "firstSeenAt",
        ] {
            assert!(manager.get(dropped).is_none(), "{dropped}");
        }
        for dropped in ["sampledAt", "diskUsedBytes"] {
            assert!(manager["latestSample"].get(dropped).is_none(), "{dropped}");
        }
        assert_eq!(body["items"][1]["availability"], "drain");
        let lone = &body["items"][2];
        assert_eq!(lone["hostname"], "worker-3");
        assert!(lone["agentVersion"].is_null() && lone["lastCheckInAt"].is_null());
        assert_eq!(lone["cpuCores"], 2, "Swarm fills what it knows");

        let (status, summary) =
            authorized_json(&router, "/api/v1/cluster/summary", &authorization).await?;
        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            summary,
            serde_json::json!({
                "nodesTotal": 4,
                "nodesActive": 2,
                "nodesDegraded": 1,
                "nodesDown": 1,
                "managersTotal": 1,
                "managersReachable": 1,
                "quorum": "healthy",
                "cpuPercent": 20.0,
                "memoryUsedBytes": 2_u64 << 30,
                "memoryTotalBytes": 16_u64 << 30,
                "containersRunning": 6,
            })
        );

        *fake.0.lock().unwrap() = Err(ManagerError::QuorumUnavailable);
        let (status, body) = authorized_json(&router, "/api/v1/nodes", &authorization).await?;
        assert_eq!(status, axum::http::StatusCode::OK, "membership still reads");
        assert_eq!(body["items"].as_array().unwrap().len(), 3);
        assert!(
            body["items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|node| node["role"] == "unknown" && node["health"] != "active")
        );
        let (_, summary) =
            authorized_json(&router, "/api/v1/cluster/summary", &authorization).await?;
        assert_eq!(summary["quorum"], "degraded");
        assert!(
            summary["managersTotal"].is_null() && summary["managersReachable"].is_null(),
            "unknown, not zero"
        );
        assert_eq!(summary["nodesActive"], 0);
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}
