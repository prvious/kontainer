use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "principals")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub kind: String,
    pub status: String,
    pub authorization_epoch: i32,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::session_family::Entity")]
    SessionFamilies,
    #[sea_orm(has_many = "super::security_audit_intent::Entity")]
    SecurityAuditIntents,
}

impl Related<super::session_family::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SessionFamilies.def()
    }
}

impl Related<super::security_audit_intent::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SecurityAuditIntents.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
