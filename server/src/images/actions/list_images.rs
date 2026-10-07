use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};
use tokio_util::sync::CancellationToken;

use crate::{
    images::domain::{ImageRow, merge_images},
    nodes::{
        actions::list_nodes,
        agents::{AgentDirectory, NodeEndpoint},
        domain::NodeStatus,
        fan_out::{NodeFailure, NodeOutcome, PER_NODE_DEADLINE, fan_out},
    },
};

pub struct ImageList {
    pub items: Vec<ImageRow>,
    pub outcomes: Vec<NodeOutcome<()>>,
    pub snapshot_at: DateTime<Utc>,
}

/// Every image on the answering members, merged by id; offline members are `unavailable`.
pub async fn execute(
    database: &DatabaseConnection,
    directory: &dyn AgentDirectory,
    cancellation: &CancellationToken,
) -> Result<ImageList, DbErr> {
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
    let outcomes = fan_out(
        endpoints,
        directory,
        PER_NODE_DEADLINE,
        |client| async move { client.list_images(cancellation).await },
    )
    .await;
    let mut per_node = Vec::new();
    let mut summary = Vec::new();
    for outcome in outcomes {
        let result = match outcome.result {
            Ok(images) => {
                per_node.push((outcome.node_id.clone(), images));
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
    Ok(ImageList {
        items: merge_images(per_node),
        outcomes: summary,
        snapshot_at,
    })
}
