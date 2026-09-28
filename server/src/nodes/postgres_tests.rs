//! Membership against real PostgreSQL: check-in upserts, samples, and derived status.

use std::error::Error;

use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
};

use super::{
    actions::{database_now, list_nodes, record_check_in},
    domain::{CheckIn, NodeStatus, SampleReading},
    entities::{node, node_sample},
};
use crate::test_support::PostgresTestDatabase;

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
        agent_started_at: Utc::now(),
        sample,
    }
}

pub(crate) fn reading(at: chrono::DateTime<Utc>, cpu: f32, disk: Option<u64>) -> SampleReading {
    SampleReading {
        sampled_at: at,
        cpu_percent: cpu,
        memory_used_bytes: 1 << 30,
        disk_used_bytes: disk,
        containers_running: 3,
        containers_paused: 0,
        containers_exited: 1,
        net_rx_bytes_per_sec: Some(100.0),
        net_tx_bytes_per_sec: None,
    }
}

#[tokio::test]
async fn postgres_check_in_upserts_the_node_and_carries_disk_forward() -> Result<(), Box<dyn Error>>
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
            check_in("manager", Some(reading(now, 10.0, Some(5 << 30)))),
        )
        .await?;
        let first = node::Entity::find_by_id("manager").one(db).await?.unwrap();

        let mut update = check_in(
            "manager",
            Some(reading(now + Duration::seconds(5), 20.0, None)),
        );
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
        assert_eq!(second.first_seen_at, first.first_seen_at);
        assert!(second.last_check_in_at >= first.last_check_in_at);

        let samples = node_sample::Entity::find()
            .filter(node_sample::Column::NodeId.eq("manager"))
            .order_by_asc(node_sample::Column::SampledAt)
            .all(db)
            .await?;
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[1].disk_used_bytes, Some(5 << 30), "carried forward");
        assert_eq!(samples[1].cpu_percent, 20.0);

        // A retry of the same sample time keeps the first row.
        record_check_in::execute(
            db,
            "development",
            check_in("manager", Some(reading(now, 99.0, None))),
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
            check_in(
                "manager",
                Some(reading(now + Duration::hours(1), 1.0, None)),
            ),
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
            check_in(
                "fresh",
                Some(reading(now - Duration::seconds(10), 5.0, None)),
            ),
        )
        .await?;
        record_check_in::execute(
            db,
            "development",
            check_in("fresh", Some(reading(now, 7.5, None))),
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
