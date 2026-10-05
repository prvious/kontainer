use sea_orm::entity::prelude::*;

/// One agent that has checked in. The id is the node id from its certificate identity.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "nodes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub cluster_id: String,
    pub hostname: String,
    pub swarm_node_id: Option<String>,
    pub advertise_address: String,
    pub agent_version: String,
    pub docker_version: String,
    pub os: String,
    pub arch: String,
    pub cpu_cores: i32,
    pub memory_total_bytes: i64,
    pub last_check_in_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::node_sample::Entity")]
    NodeSample,
}

impl Related<super::node_sample::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::NodeSample.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
