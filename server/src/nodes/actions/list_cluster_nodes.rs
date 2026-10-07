use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};
use tokio_util::sync::CancellationToken;

use crate::{
    nodes::{
        actions::list_nodes,
        domain::{ClusterNode, join_nodes},
    },
    services::actions::list_services::MANAGER_READ_DEADLINE,
    swarm::engine::{ManagerEngine, ManagerError},
};

/// Membership joined to the manager's Swarm members.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterNodes {
    pub nodes: Vec<ClusterNode>,
    /// False when there is no manager or it did not answer; every Swarm field is then unknown.
    pub swarm_answered: bool,
    pub snapshot_at: DateTime<Utc>,
}

/// Read membership and Swarm members together. Only persistence failing fails the read; a
/// missing or silent manager leaves the agents with unknown Swarm state.
pub async fn execute(
    database: &DatabaseConnection,
    manager: Option<&dyn ManagerEngine>,
    cancellation: &CancellationToken,
) -> Result<ClusterNodes, DbErr> {
    let swarm = async {
        let Some(manager) = manager else {
            return Err(ManagerError::Unavailable);
        };
        tokio::time::timeout(
            MANAGER_READ_DEADLINE,
            manager.list_swarm_nodes(cancellation),
        )
        .await
        .unwrap_or(Err(ManagerError::Unavailable))
    };
    let (agents, swarm) = tokio::join!(list_nodes::execute(database), swarm);
    let (agents, snapshot_at) = agents?;
    let swarm = swarm
        .inspect_err(|error| {
            tracing::warn!(%error, "Swarm manager did not answer: Swarm fields and manager counts are unknown");
        })
        .ok();
    Ok(ClusterNodes {
        swarm_answered: swarm.is_some(),
        nodes: join_nodes(agents, swarm),
        snapshot_at,
    })
}
