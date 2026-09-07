use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub kind: String,
    pub state: String,
    pub claimed_by: Option<String>,
    pub claimed_at: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::idempotency_record::Entity")]
    IdempotencyRecords,
    #[sea_orm(has_many = "super::operation_event::Entity")]
    Events,
    #[sea_orm(has_one = "super::audit_intent::Entity")]
    AuditIntent,
}

impl Related<super::idempotency_record::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::IdempotencyRecords.def()
    }
}

impl Related<super::operation_event::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Events.def()
    }
}

impl Related<super::audit_intent::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AuditIntent.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
