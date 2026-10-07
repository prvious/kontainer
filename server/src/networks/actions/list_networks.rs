use std::sync::Arc;

use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};
use tokio_util::sync::CancellationToken;

use crate::{
    networks::domain::{NetworkRow, merge_networks},
    nodes::{
        actions::list_nodes,
        agents::{AgentDirectory, NodeEndpoint},
        domain::NodeStatus,
        fan_out::{NodeFailure, NodeOutcome, PER_NODE_DEADLINE, fan_out},
    },
    swarm::engine::{ManagerEngine, ManagerError},
};

/// How the manager answered the cluster-wide half of the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagerStatus {
    Ok,
    /// The configured Docker host is not a Swarm manager.
    NotAManager,
    /// It is, but did not answer this time.
    Unavailable,
}

impl ManagerStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::NotAManager => "not_a_manager",
            Self::Unavailable => "unavailable",
        }
    }
}

pub struct NetworkList {
    pub items: Vec<NetworkRow>,
    pub outcomes: Vec<NodeOutcome<()>>,
    pub manager: ManagerStatus,
    pub snapshot_at: DateTime<Utc>,
}

/// The merged list: the manager's swarm-scope networks and every answering node's own.
pub async fn execute(
    database: &DatabaseConnection,
    directory: &dyn AgentDirectory,
    manager: Option<&Arc<dyn ManagerEngine>>,
    cancellation: &CancellationToken,
) -> Result<NetworkList, DbErr> {
    let (nodes, snapshot_at) = list_nodes::execute(database).await?;
    let (reachable, offline): (Vec<_>, Vec<_>) = nodes
        .into_iter()
        .partition(|node| node.status != NodeStatus::Offline);
    let endpoints = reachable
        .into_iter()
        .map(|node| NodeEndpoint {
            node_id: node.id,
            advertise_address: node.advertise_address,
        })
        .collect();
    let from_nodes = fan_out(
        endpoints,
        directory,
        PER_NODE_DEADLINE,
        |client| async move { client.list_networks(cancellation).await },
    );
    let from_manager = async {
        match manager {
            Some(manager) => {
                match tokio::time::timeout(PER_NODE_DEADLINE, manager.list_networks(cancellation))
                    .await
                {
                    Ok(Ok(networks)) => (Some(networks), ManagerStatus::Ok),
                    Ok(Err(ManagerError::NotAManager)) => (None, ManagerStatus::NotAManager),
                    _ => (None, ManagerStatus::Unavailable),
                }
            }
            None => (None, ManagerStatus::NotAManager),
        }
    };
    let (outcomes, (swarm, manager_status)) = tokio::join!(from_nodes, from_manager);
    let mut per_node = Vec::new();
    let mut summary = Vec::new();
    for outcome in outcomes {
        let result = match outcome.result {
            Ok(networks) => {
                per_node.push((outcome.node_id.clone(), networks));
                Ok(())
            }
            Err(failure) => Err(failure),
        };
        summary.push(NodeOutcome {
            node_id: outcome.node_id,
            result,
        });
    }
    summary.extend(offline.into_iter().map(|node| NodeOutcome {
        node_id: node.id,
        result: Err(NodeFailure::Unavailable),
    }));
    Ok(NetworkList {
        items: merge_networks(swarm, per_node),
        outcomes: summary,
        manager: manager_status,
        snapshot_at,
    })
}
