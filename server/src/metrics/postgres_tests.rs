//! Bucketed samples, pruning, and settings against real PostgreSQL.

use std::error::Error;

use chrono::Duration;
use sea_orm::{ActiveValue::Set, EntityTrait, PaginatorTrait};

use super::{
    actions::{node_samples, prune_samples},
    domain::SampleRange,
};
use crate::{
    nodes::{
        actions::{database_now, record_check_in},
        entities::node_sample,
        postgres_tests::{check_in, reading},
    },
    settings::actions::{
        get_settings,
        update_settings::{self, SettingsPatch, UpdateSettingsError},
    },
    test_support::PostgresTestDatabase,
};

async fn insert_sample(
    db: &sea_orm::DatabaseConnection,
    node_id: &str,
    at: chrono::DateTime<chrono::Utc>,
    cpu: f32,
    memory: i64,
) -> Result<(), sea_orm::DbErr> {
    node_sample::Entity::insert(node_sample::ActiveModel {
        node_id: Set(node_id.to_owned()),
        sampled_at: Set(at),
        cpu_percent: Set(cpu),
        memory_used_bytes: Set(memory),
        containers_running: Set(2),
        containers_paused: Set(0),
        containers_exited: Set(0),
        net_rx_bytes_per_sec: Set(Some(10.0)),
        net_tx_bytes_per_sec: Set(None),
    })
    .exec_without_returning(db)
    .await
    .map(|_| ())
}

#[tokio::test]
async fn postgres_buckets_average_per_node_and_leave_gaps_null() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let db = &database.connection;
        for node_id in ["manager", "worker-1"] {
            record_check_in::execute(db, "development", check_in(node_id, None)).await?;
        }
        let range = SampleRange::parse("5m", "60s")?;
        let starts = range.bucket_starts(database_now(db).await?);
        // Bucket 0: manager twice (10 and 30 → 20), worker-1 once (40). Bucket 2: manager only.
        insert_sample(db, "manager", starts[0] + Duration::seconds(5), 10.0, 100).await?;
        insert_sample(db, "manager", starts[0] + Duration::seconds(35), 30.0, 300).await?;
        insert_sample(
            db,
            "worker-1",
            starts[0] + Duration::seconds(20),
            40.0,
            1000,
        )
        .await?;
        insert_sample(db, "manager", starts[2] + Duration::seconds(1), 50.0, 500).await?;

        let cluster = node_samples::execute(db, range).await?;
        assert_eq!(cluster.len(), 5);
        assert_eq!(cluster[0].at, starts[0]);
        assert_eq!(cluster[0].nodes, 2, "contributors counted per bucket");
        assert_eq!(
            cluster[0].cpu_percent,
            Some(30.0),
            "mean of manager's own mean (20) and 40"
        );
        assert_eq!(cluster[0].memory_used_bytes, Some(1200.0), "200 + 1000");
        assert_eq!(cluster[0].net_rx_bytes_per_sec, Some(20.0));
        assert_eq!(cluster[0].net_tx_bytes_per_sec, None);
        assert_eq!(cluster[1].cpu_percent, None, "a gap is null, never 0");
        assert_eq!(cluster[1].memory_used_bytes, None);
        assert_eq!(cluster[1].nodes, 0);
        assert_eq!(cluster[2].nodes, 1);
        assert_eq!(cluster[2].cpu_percent, Some(50.0));
        assert_eq!(cluster[3].cpu_percent, None);

        // 24h at 15m is 96 buckets; before the agents started, all null.
        let day = SampleRange::parse("24h", "15m")?;
        let series = node_samples::execute(db, day).await?;
        assert_eq!(series.len(), 96);
        assert!(series[..90].iter().all(|point| point.cpu_percent.is_none()));
        assert!(series.iter().any(|point| point.cpu_percent.is_some()));
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_prune_removes_only_samples_past_retention() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let db = &database.connection;
        record_check_in::execute(db, "development", check_in("manager", None)).await?;
        let now = database_now(db).await?;
        for days in [0, 2, 6, 8, 30] {
            insert_sample(db, "manager", now - Duration::days(days), 1.0, 1).await?;
        }
        assert_eq!(get_settings::execute(db).await?.metrics_retention_days, 7);
        assert_eq!(prune_samples::execute(db).await?, 2);
        assert_eq!(node_sample::Entity::find().count(db).await?, 3);

        update_settings::execute(
            db,
            SettingsPatch {
                metrics_retention_days: Some(1),
            },
        )
        .await?;
        assert_eq!(prune_samples::execute(db).await?, 2);
        assert_eq!(node_sample::Entity::find().count(db).await?, 1);
        assert_eq!(prune_samples::execute(db).await?, 0, "idempotent");
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_settings_validate_retention_and_persist() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let db = &database.connection;
        for invalid in [0, -1, 366, i32::MAX] {
            let error = update_settings::execute(
                db,
                SettingsPatch {
                    metrics_retention_days: Some(invalid),
                },
            )
            .await
            .unwrap_err();
            assert!(matches!(error, UpdateSettingsError::RetentionOutOfRange));
        }
        assert_eq!(get_settings::execute(db).await?.metrics_retention_days, 7);
        let updated = update_settings::execute(
            db,
            SettingsPatch {
                metrics_retention_days: Some(30),
            },
        )
        .await?;
        assert_eq!(updated.metrics_retention_days, 30);
        let unchanged = update_settings::execute(db, SettingsPatch::default()).await?;
        assert_eq!(unchanged.metrics_retention_days, 30);
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_samples_record_through_check_in() -> Result<(), Box<dyn Error>> {
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let evidence = async {
        let db = &database.connection;
        let now = database_now(db).await?;
        record_check_in::execute(
            db,
            "development",
            check_in("manager", Some(reading(now, 12.0))),
        )
        .await?;
        let range = SampleRange::parse("5m", "5s")?;
        let points = node_samples::execute(db, range).await?;
        assert_eq!(points.len(), 60);
        let filled: Vec<_> = points
            .iter()
            .filter(|point| point.cpu_percent.is_some())
            .collect();
        assert_eq!(filled.len(), 1);
        assert_eq!(filled[0].cpu_percent, Some(12.0));
        assert_eq!(filled[0].containers_running, Some(3.0));
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let removed = database.cleanup().await?;
    evidence?;
    assert!(removed);
    Ok(())
}
