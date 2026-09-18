use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "security_audit_intents")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub principal_id: Uuid,
    pub family_id: Uuid,
    pub action: String,
    pub state: String,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::principal::Entity",
        from = "Column::PrincipalId",
        to = "super::principal::Column::Id",
        on_update = "NoAction",
        on_delete = "Restrict"
    )]
    Principal,
    #[sea_orm(
        belongs_to = "super::session_family::Entity",
        from = "Column::FamilyId",
        to = "super::session_family::Column::Id",
        on_update = "NoAction",
        on_delete = "Restrict"
    )]
    SessionFamily,
}

impl Related<super::principal::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Principal.def()
    }
}

impl Related<super::session_family::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SessionFamily.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
