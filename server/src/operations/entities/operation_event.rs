use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "operation_events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub operation_id: Uuid,
    pub sequence: i32,
    pub state: String,
    pub error_code: Option<String>,
    pub created_at: DateTimeUtc,
    /// Progress or result document for the kind.
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub detail: Option<Json>,
    /// Human-readable failure message; never daemon or transport internals.
    pub message: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::operation::Entity",
        from = "Column::OperationId",
        to = "super::operation::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Operation,
}

impl Related<super::operation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Operation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
