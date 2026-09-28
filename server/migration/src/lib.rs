#![forbid(unsafe_code)]

mod m20260823_000001_create_durable_operations;
mod m20260917_000002_create_server_sessions;
mod m20260918_000003_create_github_bootstrap_login;
mod m20260928_000004_add_password_auth;
mod m20260928_000005_add_node_membership;

pub use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260823_000001_create_durable_operations::Migration),
            Box::new(m20260917_000002_create_server_sessions::Migration),
            Box::new(m20260918_000003_create_github_bootstrap_login::Migration),
            Box::new(m20260928_000004_add_password_auth::Migration),
            Box::new(m20260928_000005_add_node_membership::Migration),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::{Migrator, MigratorTrait};

    #[test]
    fn durable_operation_migration_is_registered() {
        assert_eq!(Migrator::migrations().len(), 5);
    }
}
