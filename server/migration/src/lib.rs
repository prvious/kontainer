#![forbid(unsafe_code)]

pub use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Migrator, MigratorTrait};

    #[test]
    fn greenfield_migrator_starts_empty() {
        assert!(Migrator::migrations().is_empty());
    }
}
