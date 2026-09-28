use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};
use tokio_util::sync::CancellationToken;

use crate::{
    nodes::{
        actions::list_nodes,
        agents::{AgentDirectory, NodeEndpoint},
        domain::NodeStatus,
        fan_out::{NodeFailure, NodeOutcome, PER_NODE_DEADLINE, fan_out},
    },
    volumes::domain::VolumeRow,
};

pub struct VolumeList {
    pub items: Vec<VolumeRow>,
    pub outcomes: Vec<NodeOutcome<()>>,
    pub snapshot_at: DateTime<Utc>,
}

/// Narrows the list to one node, one name, or both (the create dialog's conflict check).
#[derive(Debug, Clone, Copy, Default)]
pub struct VolumeFilter<'a> {
    pub node_id: Option<&'a str>,
    pub name: Option<&'a str>,
}

/// Why a filtered list could not be produced.
#[derive(Debug)]
pub enum ListVolumesError {
    Persistence(DbErr),
    /// The filter names a node that is not a member.
    UnknownNode,
}

/// Volumes on the answering members (or the one filtered node), sorted by node then name;
/// offline members are `unavailable` without a dial.
pub async fn execute(
    database: &DatabaseConnection,
    directory: &dyn AgentDirectory,
    filter: VolumeFilter<'_>,
    cancellation: &CancellationToken,
) -> Result<VolumeList, ListVolumesError> {
    let (mut nodes, snapshot_at) = list_nodes::execute(database)
        .await
        .map_err(ListVolumesError::Persistence)?;
    if let Some(node_id) = filter.node_id {
        nodes.retain(|node| node.id == node_id);
        if nodes.is_empty() {
            return Err(ListVolumesError::UnknownNode);
        }
    }
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
    let outcomes = fan_out(
        endpoints,
        directory,
        PER_NODE_DEADLINE,
        |client| async move { client.list_volumes(cancellation).await },
    )
    .await;
    let mut items = Vec::new();
    let mut summary = Vec::new();
    for outcome in outcomes {
        let result = match outcome.result {
            Ok(volumes) => {
                items.extend(
                    volumes
                        .into_iter()
                        .filter(|volume| filter.name.is_none_or(|name| volume.name == name))
                        .map(|volume| VolumeRow::from_agent(&outcome.node_id, volume)),
                );
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
    items.sort_by(|left, right| (&left.node_id, &left.name).cmp(&(&right.node_id, &right.name)));
    Ok(VolumeList {
        items,
        outcomes: summary,
        snapshot_at,
    })
}
