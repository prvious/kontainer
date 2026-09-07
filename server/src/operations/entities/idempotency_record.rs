use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "idempotency_records")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub principal_ref: String,
    pub scope: String,
    pub key_hash: String,
    pub request_hash: String,
    pub operation_id: Uuid,
    pub expires_at: DateTimeUtc,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::operation::Entity",
        from = "Column::OperationId",
        to = "super::operation::Column::Id",
        on_update = "NoAction",
        on_delete = "Restrict"
    )]
    Operation,
}

impl Related<super::operation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Operation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
