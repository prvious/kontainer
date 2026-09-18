use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "github_identities")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub github_account_id: i64,
    pub principal_id: Uuid,
    pub login: String,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    pub last_authenticated_at: DateTimeUtc,
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
}

impl Related<super::principal::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Principal.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
