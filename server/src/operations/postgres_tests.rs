use std::{env, error::Error, io, path::Path, time::Duration as StdDuration};

use axum::{Router, body::Body, body::to_bytes, http::Request, routing::get};
use chrono::{Duration, Utc};
use loco_rs::{app::AppContext, config::Config, environment::Environment};
use migration::{Migrator, MigratorTrait, SchemaManager};
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait, PaginatorTrait,
    QueryResult, Statement, TransactionTrait,
};
use tower::ServiceExt;
use url::Url;
use uuid::Uuid;

use super::{
    actions::{admit, claim_next, get, mark_uncertain},
    domain::{AdmitOperationRequest, OperationState, PrincipalContext},
    entities::{audit_intent, idempotency_record, operation, operation_event},
};

const TEST_DATABASE_URL: &str = "KONTAINER_TEST_DATABASE_URL";
const REQUIRE_POSTGRES_TESTS: &str = "KONTAINER_REQUIRE_POSTGRES_TESTS";

struct PostgresTestDatabase {
    admin_url: Url,
    database_name: String,
    connection: DatabaseConnection,
}

impl PostgresTestDatabase {
    async fn create(apply_migrations: bool) -> Result<Option<Self>, Box<dyn Error>> {
        let Ok(raw_admin_url) = env::var(TEST_DATABASE_URL) else {
            if env::var(REQUIRE_POSTGRES_TESTS).as_deref() == Ok("1") {
                return Err(io::Error::other(format!(
                    "{TEST_DATABASE_URL} is required when {REQUIRE_POSTGRES_TESTS}=1"
                ))
                .into());
            }
            eprintln!("skipping PostgreSQL integration test: {TEST_DATABASE_URL} is not set");
            return Ok(None);
        };

        let mut admin_url = Url::parse(&raw_admin_url)?;
        admin_url.set_path("/postgres");
        let database_name = format!("kontainer_test_{}", Uuid::now_v7().simple());
        let admin = Database::connect(admin_url.as_str()).await?;

        // PostgreSQL does not parameterize database identifiers and SeaQuery does
        // not model CREATE DATABASE. The name is generated internally from a UUID,
        // then quoted, so this test-only administrative statement has no caller input.
        admin
            .execute_raw(Statement::from_string(
                DbBackend::Postgres,
                format!("CREATE DATABASE \"{database_name}\""),
            ))
            .await?;
        admin.close().await?;

        let mut database_url = admin_url.clone();
        database_url.set_path(&format!("/{database_name}"));
        let connection = Database::connect(database_url.as_str()).await?;
        if apply_migrations {
            Migrator::up(&connection, None).await?;
        }

        Ok(Some(Self {
            admin_url,
            database_name,
            connection,
        }))
    }

    async fn cleanup(self) -> Result<bool, Box<dyn Error>> {
        self.connection.close().await?;
        let admin = Database::connect(self.admin_url.as_str()).await?;

        // FORCE makes cleanup deterministic if a failed assertion left a pooled
        // test connection alive. The database name is the UUID-derived value above.
        admin
            .execute_raw(Statement::from_string(
                DbBackend::Postgres,
                format!("DROP DATABASE \"{}\" WITH (FORCE)", self.database_name),
            ))
            .await?;
        let remaining: Option<QueryResult> = admin
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT 1 FROM pg_database WHERE datname = $1",
                [self.database_name.into()],
            ))
            .await?;
        admin.close().await?;

        Ok(remaining.is_none())
    }
}

fn admission_request(
    idempotency_key: &str,
    canonical_request: &[u8],
) -> Result<AdmitOperationRequest, Box<dyn Error>> {
    Ok(AdmitOperationRequest::new(
        PrincipalContext::new("bootstrap:integration-test")?,
        "operations.integration",
        idempotency_key,
        "test.operation",
        "operation.admit",
        Some("test-target".to_owned()),
        canonical_request.to_vec(),
        Utc::now() + Duration::hours(1),
    )?)
}

#[tokio::test]
async fn postgres_migration_up_down_status_and_cleanup() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(false).await? else {
        return Ok(());
    };

    let evidence = async {
        let pending_before = Migrator::get_pending_migrations(&test_database.connection)
            .await?
            .len();
        Migrator::status(&test_database.connection).await?;
        Migrator::up(&test_database.connection, None).await?;
        let applied_after_up = Migrator::get_applied_migrations(&test_database.connection)
            .await?
            .len();
        let schema = SchemaManager::new(&test_database.connection);
        let tables_exist_after_up = [
            schema.has_table("operations").await?,
            schema.has_table("idempotency_records").await?,
            schema.has_table("operation_events").await?,
            schema.has_table("audit_intents").await?,
        ];

        Migrator::down(&test_database.connection, None).await?;
        Migrator::status(&test_database.connection).await?;
        let pending_after_down = Migrator::get_pending_migrations(&test_database.connection)
            .await?
            .len();
        let tables_exist_after_down = [
            schema.has_table("operations").await?,
            schema.has_table("idempotency_records").await?,
            schema.has_table("operation_events").await?,
            schema.has_table("audit_intents").await?,
        ];

        Ok::<_, sea_orm::DbErr>((
            pending_before,
            applied_after_up,
            pending_after_down,
            tables_exist_after_up,
            tables_exist_after_down,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (pending_before, applied_after_up, pending_after_down, tables_after_up, tables_after_down) =
        evidence?;

    assert_eq!(pending_before, 1);
    assert_eq!(applied_after_up, 1);
    assert_eq!(pending_after_down, 1);
    assert!(tables_after_up.into_iter().all(|exists| exists));
    assert!(tables_after_down.into_iter().all(|exists| !exists));
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_admission_is_atomic_and_idempotent() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let first = admit::execute(
            &test_database.connection,
            admission_request("admit-once", br#"{"replicas":2}"#)?,
        )
        .await?;
        let replay = admit::execute(
            &test_database.connection,
            admission_request("admit-once", br#"{"replicas":2}"#)?,
        )
        .await?;
        let conflict = admit::execute(
            &test_database.connection,
            admission_request("admit-once", br#"{"replicas":3}"#)?,
        )
        .await;
        let snapshot = get::execute(&test_database.connection, first.operation.id).await?;

        Ok::<_, Box<dyn Error>>((
            first,
            replay,
            conflict,
            snapshot,
            operation::Entity::find()
                .count(&test_database.connection)
                .await?,
            idempotency_record::Entity::find()
                .count(&test_database.connection)
                .await?,
            operation_event::Entity::find()
                .count(&test_database.connection)
                .await?,
            audit_intent::Entity::find()
                .count(&test_database.connection)
                .await?,
        ))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (first, replay, conflict, snapshot, operations, idempotency, events, audit) = evidence?;

    assert!(!first.replayed);
    assert!(replay.replayed);
    assert_eq!(first.operation.id, replay.operation.id);
    assert!(matches!(
        conflict,
        Err(admit::AdmitOperationError::IdempotencyConflict { operation_id })
            if operation_id == first.operation.id
    ));
    assert_eq!(snapshot.operation.state, OperationState::Recoverable);
    assert_eq!(snapshot.last_event_sequence, 1);
    assert_eq!((operations, idempotency, events, audit), (1, 1, 1, 1));
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_operation_is_polled_through_the_versioned_http_route()
-> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let admitted = admit::execute(
            &test_database.connection,
            admission_request("poll-through-http", b"poll")?,
        )
        .await?;
        let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config");
        let config = Config::from_folder(&Environment::Test, &config_path)?;
        let context =
            AppContext::builder(Environment::Test, test_database.connection.clone(), config)
                .build();
        let application = Router::new()
            .route(
                "/api/v1/operations/{operation_id}",
                get(crate::controllers::operations::show),
            )
            .with_state(context);
        let response = application
            .oneshot(
                Request::get(format!("/api/v1/operations/{}", admitted.operation.id))
                    .body(Body::empty())?,
            )
            .await?;
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await?;
        let document: serde_json::Value = serde_json::from_slice(&body)?;

        Ok::<_, Box<dyn Error>>((admitted.operation.id, status, document))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (operation_id, status, document) = evidence?;

    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(document["id"], operation_id.to_string());
    assert_eq!(document["state"], OperationState::Recoverable.as_str());
    assert_eq!(document["lastEventSequence"], 1);
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_skip_locked_prevents_a_double_claim() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let admitted = admit::execute(
            &test_database.connection,
            admission_request("claim-once", b"claim")?,
        )
        .await?;

        let locking_transaction = test_database.connection.begin().await?;
        let locked = claim_next::claimable_query()
            .one(&locking_transaction)
            .await?;
        let skipped = tokio::time::timeout(
            StdDuration::from_secs(2),
            claim_next::execute(&test_database.connection, "controller:skipped"),
        )
        .await??;
        locking_transaction.rollback().await?;

        let (first_worker, second_worker) = tokio::join!(
            claim_next::execute(&test_database.connection, "controller:first"),
            claim_next::execute(&test_database.connection, "controller:second")
        );
        let first_worker = first_worker?;
        let second_worker = second_worker?;
        let claimed = [first_worker, second_worker]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        Ok::<_, Box<dyn Error>>((admitted.operation.id, locked, skipped, claimed))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (operation_id, locked, skipped, claimed) = evidence?;

    assert_eq!(locked.map(|model| model.id), Some(operation_id));
    assert!(skipped.is_none());
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].id, operation_id);
    assert_eq!(claimed[0].state, OperationState::Running);
    assert!(removed);
    Ok(())
}

#[tokio::test]
async fn postgres_rollback_and_uncertain_recovery_are_explicit() -> Result<(), Box<dyn Error>> {
    let Some(test_database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };

    let evidence = async {
        let injected = admit::execute_with_injected_rollback(
            &test_database.connection,
            admission_request("must-rollback", b"rollback")?,
        )
        .await;
        let counts_after_rollback = (
            operation::Entity::find()
                .count(&test_database.connection)
                .await?,
            idempotency_record::Entity::find()
                .count(&test_database.connection)
                .await?,
            operation_event::Entity::find()
                .count(&test_database.connection)
                .await?,
            audit_intent::Entity::find()
                .count(&test_database.connection)
                .await?,
        );

        let admitted = admit::execute(
            &test_database.connection,
            admission_request("becomes-uncertain", b"timeout")?,
        )
        .await?;
        let recoverable = get::execute(&test_database.connection, admitted.operation.id).await?;
        claim_next::execute(&test_database.connection, "controller:worker")
            .await?
            .ok_or_else(|| io::Error::other("expected admitted operation to be claimed"))?;
        mark_uncertain::execute(
            &test_database.connection,
            admitted.operation.id,
            "engine_timeout",
        )
        .await?;
        let uncertain = get::execute(&test_database.connection, admitted.operation.id).await?;

        Ok::<_, Box<dyn Error>>((injected, counts_after_rollback, recoverable, uncertain))
    }
    .await;
    let removed = test_database.cleanup().await?;
    let (injected, counts_after_rollback, recoverable, uncertain) = evidence?;

    assert!(matches!(
        injected,
        Err(admit::AdmitOperationError::InjectedFailure)
    ));
    assert_eq!(counts_after_rollback, (0, 0, 0, 0));
    assert_eq!(recoverable.operation.state, OperationState::Recoverable);
    assert_eq!(uncertain.operation.state, OperationState::Uncertain);
    assert_eq!(uncertain.last_event_sequence, 3);
    assert_eq!(uncertain.last_error_code.as_deref(), Some("engine_timeout"));
    assert!(removed);
    Ok(())
}
