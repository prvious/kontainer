use sea_orm::entity::prelude::*;

/// The single controller settings row (`id = 1`), seeded by its migration.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "controller_settings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i16,
    pub metrics_retention_days: i32,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
