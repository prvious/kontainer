//! Test-only PostgreSQL scratch-database helper.
//!
//! Each test creates an isolated database, optionally runs migrations, and
//! drops it afterwards. Production code must never depend on this module.

use std::{env, error::Error, io};

use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, QueryResult, Statement};
use url::Url;
use uuid::Uuid;

const TEST_DATABASE_URL: &str = "KONTAINER_TEST_DATABASE_URL";
const REQUIRE_POSTGRES_TESTS: &str = "KONTAINER_REQUIRE_POSTGRES_TESTS";

pub struct PostgresTestDatabase {
    admin_url: Url,
    database_name: String,
    pub connection: DatabaseConnection,
}

impl PostgresTestDatabase {
    pub async fn create(apply_migrations: bool) -> Result<Option<Self>, Box<dyn Error>> {
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

    pub async fn cleanup(self) -> Result<bool, Box<dyn Error>> {
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
