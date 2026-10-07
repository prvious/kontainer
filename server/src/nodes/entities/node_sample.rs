use sea_orm::entity::prelude::*;

/// One resource sample from a check-in.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "node_samples")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub node_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub sampled_at: DateTimeUtc,
    pub cpu_percent: f32,
    pub memory_used_bytes: i64,
    pub containers_running: i32,
    pub containers_paused: i32,
    pub containers_exited: i32,
    pub net_rx_bytes_per_sec: Option<f64>,
    pub net_tx_bytes_per_sec: Option<f64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::node::Entity",
        from = "Column::NodeId",
        to = "super::node::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Node,
}

impl Related<super::node::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Node.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
