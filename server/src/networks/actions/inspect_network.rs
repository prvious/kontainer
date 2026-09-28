use sea_orm::DatabaseConnection;
use tokio_util::sync::CancellationToken;

use crate::{
    nodes::{
        actions::list_nodes,
        agents::{AgentDirectory, NodeEndpoint},
        domain::NodeStatus,
        fan_out::{PER_NODE_DEADLINE, fan_out},
    },
    rpc::AgentClientError,
};

/// One attached container, with the node it runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachedContainer {
    pub node_id: String,
    pub container_id: String,
    pub name: String,
}

#[derive(Debug)]
pub enum InspectNetworkError {
    Persistence,
    /// The one node asked about is not a member, or doesn't have the network.
    NotFound,
    Agent(AgentClientError),
}

/// The containers attached to one network: on `node` for a local network, or on every reachable
/// node for a swarm-scope one (a manager only sees its own endpoints, so the nodes are asked).
pub async fn execute(
    database: &DatabaseConnection,
    directory: &dyn AgentDirectory,
    network_id: &str,
    node: Option<&str>,
    cancellation: &CancellationToken,
) -> Result<Vec<AttachedContainer>, InspectNetworkError> {
    let (nodes, _) = list_nodes::execute(database)
        .await
        .map_err(|_| InspectNetworkError::Persistence)?;
    let targets: Vec<NodeEndpoint> = nodes
        .into_iter()
        .filter(|member| node.is_none_or(|node| member.id == node))
        .filter(|member| node.is_some() || member.status != NodeStatus::Offline)
        .map(|member| NodeEndpoint {
            node_id: member.id,
            advertise_address: member.advertise_address,
        })
        .collect();
    if node.is_some() && targets.is_empty() {
        return Err(InspectNetworkError::NotFound);
    }
    let outcomes = fan_out(targets, directory, PER_NODE_DEADLINE, |client| async move {
        client.inspect_network(network_id, cancellation).await
    })
    .await;
    let mut attached = Vec::new();
    for outcome in outcomes {
        match outcome.result {
            Ok(detail) => {
                attached.extend(
                    detail
                        .endpoints
                        .into_iter()
                        .map(|endpoint| AttachedContainer {
                            node_id: outcome.node_id.clone(),
                            container_id: endpoint.container_id,
                            name: endpoint.name,
                        }),
                )
            }
            // A swarm network only exists on the nodes running its tasks.
            Err(crate::nodes::fan_out::NodeFailure::Error("not_found")) if node.is_none() => {}
            Err(crate::nodes::fan_out::NodeFailure::Error("not_found")) => {
                return Err(InspectNetworkError::NotFound);
            }
            Err(_) if node.is_none() => {}
            Err(crate::nodes::fan_out::NodeFailure::Timeout) => {
                return Err(InspectNetworkError::Agent(AgentClientError::TimedOut));
            }
            Err(_) => return Err(InspectNetworkError::Agent(AgentClientError::Unavailable)),
        }
    }
    attached.sort_by(|left, right| (&left.name, &left.node_id).cmp(&(&right.name, &right.node_id)));
    Ok(attached)
}
