use std::{future::Future, time::Duration};

use axum::{
    Json,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};
use loco_rs::app::AppContext;
use migration::MigratorTrait;
use sea_orm::DbErr;
use serde::Serialize;
use tokio::time::{Instant, timeout, timeout_at};
use utoipa::ToSchema;

use crate::{
    auth::domain::PrincipalContext,
    rpc::{AgentClientError, AgentDescription, SharedAgentClient},
    server::ForceCancellation,
};

const CHECK_DEADLINE: Duration = Duration::from_secs(4);
const DATABASE_DEADLINE: Duration = Duration::from_secs(1);

#[derive(Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeepStatusResponse {
    build: BuildResponse,
    migration: MigrationState,
    database: DatabaseState,
    agent_rpc: AgentRpcState,
}

#[derive(Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BuildResponse {
    version: &'static str,
}

#[derive(Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrationState {
    Current,
    Unknown,
}

#[derive(Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseState {
    Reachable,
    Unreachable,
    Timeout,
}

#[derive(Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentRpcState {
    Up,
    Down,
    Timeout,
}

#[utoipa::path(
    get,
    path = "/api/v1/status/deep",
    operation_id = "deep_status",
    responses(
        (status = 200, description = "Authenticated dependency diagnostic; component failures are states, not HTTP errors", body = DeepStatusResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 503, description = "Controller draining new work or session persistence unavailable; the body is either HealthResponse or a generic error")
    ),
    security(("access_session" = []))
)]
pub(super) async fn show(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
) -> Response {
    // The handler runs only after session validation; start one deadline here.
    let deadline = Instant::now() + CHECK_DEADLINE;
    let client = context.shared_store.get::<SharedAgentClient>();
    let cancellation = context.shared_store.get::<ForceCancellation>();
    let status = run_checks(
        context.db.ping(),
        async {
            migration::Migrator::get_pending_migrations_read_only(&context.db)
                .await
                .map(|pending| !pending.is_empty())
        },
        async {
            match (client, cancellation) {
                (Some(client), Some(cancellation)) => client.0.describe(&cancellation.0).await,
                _ => Err(AgentClientError::Unavailable),
            }
        },
        deadline,
    )
    .await;
    ([(header::CACHE_CONTROL, "no-store")], Json(status)).into_response()
}

async fn run_checks<D, M, A>(
    database: D,
    migration: M,
    agent: A,
    deadline: Instant,
) -> DeepStatusResponse
where
    D: Future<Output = Result<(), DbErr>>,
    M: Future<Output = Result<bool, DbErr>>,
    A: Future<Output = Result<AgentDescription, AgentClientError>>,
{
    let database = async {
        match timeout_at(deadline, timeout(DATABASE_DEADLINE, database)).await {
            Ok(Ok(Ok(()))) => DatabaseState::Reachable,
            Ok(Ok(Err(_))) => DatabaseState::Unreachable,
            Ok(Err(_)) | Err(_) => DatabaseState::Timeout,
        }
    };
    let migration = async {
        match timeout_at(deadline, timeout(DATABASE_DEADLINE, migration)).await {
            Ok(Ok(Ok(false))) => MigrationState::Current,
            _ => MigrationState::Unknown,
        }
    };
    let agent = async {
        match timeout_at(deadline, agent).await {
            Ok(Ok(_)) => AgentRpcState::Up,
            Ok(Err(AgentClientError::TimedOut)) | Err(_) => AgentRpcState::Timeout,
            Ok(Err(_)) => AgentRpcState::Down,
        }
    };
    let (database, migration, agent_rpc) = tokio::join!(database, migration, agent);
    DeepStatusResponse {
        build: BuildResponse {
            version: env!("CARGO_PKG_VERSION"),
        },
        migration,
        database,
        agent_rpc,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::{pending, ready},
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use sea_orm::DbErr;
    use tokio::{sync::Barrier, time::Instant};

    use super::{AgentClientError, AgentDescription, run_checks};

    fn description() -> AgentDescription {
        AgentDescription {
            node_id: "secret-node-canary".to_owned(),
            agent_version: "secret-version-canary".to_owned(),
            architecture: "secret-architecture-canary".to_owned(),
            docker_connected: false,
        }
    }

    #[tokio::test]
    async fn maps_every_state_and_serializes_only_redacted_fields() {
        let deadline = Instant::now() + Duration::from_millis(50);
        let good = run_checks(
            ready(Ok(())),
            ready(Ok(false)),
            ready(Ok(description())),
            deadline,
        )
        .await;
        assert_eq!(
            serde_json::to_value(&good).unwrap(),
            serde_json::json!({
                "build": {"version": env!("CARGO_PKG_VERSION")},
                "migration": "current",
                "database": "reachable",
                "agentRpc": "up"
            })
        );

        let failed = run_checks(
            ready(Err(DbErr::Custom("secret-database-canary".into()))),
            ready(Err(DbErr::Custom("secret-migration-canary".into()))),
            ready(Err(AgentClientError::IdentityMismatch)),
            deadline,
        )
        .await;
        assert_eq!(failed.database, super::DatabaseState::Unreachable);
        assert_eq!(failed.migration, super::MigrationState::Unknown);
        assert_eq!(failed.agent_rpc, super::AgentRpcState::Down);
        let serialized = serde_json::to_string(&failed).unwrap();
        assert!(!serialized.contains("canary"));

        let pending_migration = run_checks(
            ready(Ok(())),
            ready(Ok(true)),
            ready(Err(AgentClientError::TimedOut)),
            Instant::now() + Duration::from_millis(50),
        )
        .await;
        assert_eq!(pending_migration.migration, super::MigrationState::Unknown);
        assert_eq!(pending_migration.agent_rpc, super::AgentRpcState::Timeout);

        let timed_out = run_checks(
            pending(),
            pending(),
            pending(),
            Instant::now() + Duration::from_millis(20),
        )
        .await;
        assert_eq!(timed_out.database, super::DatabaseState::Timeout);
        assert_eq!(timed_out.migration, super::MigrationState::Unknown);
        assert_eq!(timed_out.agent_rpc, super::AgentRpcState::Timeout);
    }

    #[tokio::test]
    async fn starts_all_checks_concurrently_and_drops_them_at_the_shared_deadline() {
        struct Active(Arc<AtomicUsize>);
        impl Drop for Active {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }

        let started = Arc::new(Barrier::new(3));
        let total = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let database = {
            let started = started.clone();
            let total = total.clone();
            let active = active.clone();
            async move {
                total.fetch_add(1, Ordering::SeqCst);
                active.fetch_add(1, Ordering::SeqCst);
                let _guard = Active(active);
                started.wait().await;
                pending::<Result<(), DbErr>>().await
            }
        };
        let migration = {
            let started = started.clone();
            let total = total.clone();
            let active = active.clone();
            async move {
                total.fetch_add(1, Ordering::SeqCst);
                active.fetch_add(1, Ordering::SeqCst);
                let _guard = Active(active);
                started.wait().await;
                pending::<Result<bool, DbErr>>().await
            }
        };
        let agent = {
            let started = started.clone();
            let total = total.clone();
            let active = active.clone();
            async move {
                total.fetch_add(1, Ordering::SeqCst);
                active.fetch_add(1, Ordering::SeqCst);
                let _guard = Active(active);
                started.wait().await;
                pending::<Result<AgentDescription, AgentClientError>>().await
            }
        };
        let deadline = Instant::now() + Duration::from_millis(50);
        let status = tokio::time::timeout(
            Duration::from_millis(200),
            run_checks(database, migration, agent, deadline),
        )
        .await
        .expect("all checks started before the shared deadline");
        assert_eq!(total.load(Ordering::SeqCst), 3);
        assert_eq!(status.agent_rpc, super::AgentRpcState::Timeout);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn database_and_migration_checks_have_one_second_limits() {
        let started = Instant::now();
        let status = tokio::time::timeout(
            Duration::from_secs(3),
            run_checks(
                pending(),
                pending(),
                ready(Ok(description())),
                started + Duration::from_secs(4),
            ),
        )
        .await
        .expect("database checks must finish before the shared deadline");
        assert!(started.elapsed() >= Duration::from_millis(900));
        assert_eq!(status.database, super::DatabaseState::Timeout);
        assert_eq!(status.migration, super::MigrationState::Unknown);
        assert_eq!(status.agent_rpc, super::AgentRpcState::Up);
    }

    #[tokio::test]
    async fn dropping_the_request_drops_all_checks() {
        struct Active(Arc<AtomicUsize>);
        impl Drop for Active {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let active = Arc::new(AtomicUsize::new(0));
        let wait = |active: Arc<AtomicUsize>| async move {
            active.fetch_add(1, Ordering::SeqCst);
            let _guard = Active(active);
            pending::<()>().await;
        };
        let mut checks = Box::pin(run_checks(
            async {
                wait(active.clone()).await;
                Ok(())
            },
            async {
                wait(active.clone()).await;
                Ok(false)
            },
            async {
                wait(active.clone()).await;
                Ok(description())
            },
            Instant::now() + Duration::from_secs(4),
        ));
        tokio::select! {
            _ = &mut checks => panic!("checks unexpectedly finished"),
            _ = tokio::time::sleep(Duration::from_millis(200)) => panic!("checks did not all start"),
            _ = async {
                loop {
                    if active.load(Ordering::SeqCst) == 3 { break; }
                    tokio::task::yield_now().await;
                }
            } => {}
        }
        drop(checks);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn postgres_migration_check_does_not_install_its_table()
    -> Result<(), Box<dyn std::error::Error>> {
        use migration::MigratorTrait;

        let Some(database) = crate::test_support::PostgresTestDatabase::create(false).await? else {
            return Ok(());
        };
        let evidence = async {
            let manager = migration::SchemaManager::new(&database.connection);
            assert!(!manager.has_table("seaql_migrations").await?);
            let state = run_checks(
                database.connection.ping(),
                async {
                    migration::Migrator::get_pending_migrations_read_only(&database.connection)
                        .await
                        .map(|pending| !pending.is_empty())
                },
                ready(Err(AgentClientError::Unavailable)),
                Instant::now() + Duration::from_secs(4),
            )
            .await;
            assert_eq!(state.database, super::DatabaseState::Reachable);
            assert_eq!(state.migration, super::MigrationState::Unknown);
            assert!(!manager.has_table("seaql_migrations").await?);
            Ok::<_, Box<dyn std::error::Error>>(())
        }
        .await;
        let removed = database.cleanup().await?;
        evidence?;
        assert!(removed);
        Ok(())
    }
}
