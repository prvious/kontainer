#![forbid(unsafe_code)]

mod m20260823_000001_create_durable_operations;

pub use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(
            m20260823_000001_create_durable_operations::Migration,
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::{Migrator, MigratorTrait};

    #[test]
    fn durable_operation_migration_is_registered() {
        assert_eq!(Migrator::migrations().len(), 1);
    }
}
