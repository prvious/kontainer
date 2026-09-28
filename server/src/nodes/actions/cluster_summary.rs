use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};
use tokio_util::sync::CancellationToken;

use crate::{
    nodes::{
        actions::list_cluster_nodes,
        domain::{ClusterSummary, summarize},
    },
    swarm::engine::ManagerEngine,
};

/// Node counts by health, manager reachability, quorum, and load from the same joined read the
/// node list makes.
pub async fn execute(
    database: &DatabaseConnection,
    manager: Option<&dyn ManagerEngine>,
    cancellation: &CancellationToken,
) -> Result<(ClusterSummary, DateTime<Utc>), DbErr> {
    let cluster = list_cluster_nodes::execute(database, manager, cancellation).await?;
    Ok((
        summarize(&cluster.nodes, cluster.swarm_answered),
        cluster.snapshot_at,
    ))
}
