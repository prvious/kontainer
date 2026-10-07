use std::collections::HashSet;

use sea_orm::DatabaseConnection;
use tokio_util::sync::CancellationToken;

use crate::{
    containers::actions::list_containers,
    nodes::{
        actions::list_nodes, agents::AgentDirectory, domain::departed_agent_ids,
        fan_out::NodeOutcome,
    },
    services::actions::list_services::{self, MANAGER_READ_DEADLINE},
    stacks::domain::{StackRow, group_stacks},
    swarm::engine::{ManagerEngine, ManagerError},
};

/// Every stack from the manager's services, with CPU and memory from the containers fan-out.
/// Without agents, or when any node's container list fails, the figures are left out, not the
/// stacks: a partial sum would look complete. A departed node (ADR 0037) holds no containers, so
/// its failure doesn't count.
pub async fn execute(
    database: &DatabaseConnection,
    manager: &dyn ManagerEngine,
    directory: Option<&dyn AgentDirectory>,
    cancellation: &CancellationToken,
) -> Result<Vec<StackRow>, ManagerError> {
    let services = list_services::execute(manager, None, cancellation);
    let containers = async {
        let Some(directory) = directory else {
            return Vec::new();
        };
        let (list, departed) = tokio::join!(
            list_containers::execute(database, directory, None, cancellation),
            departed_nodes(database, manager, cancellation),
        );
        match list {
            Ok(list) => {
                let failed = failed_nodes(&list.outcomes, &departed);
                if failed == 0 {
                    list.items
                } else {
                    tracing::warn!(
                        failed_nodes = failed,
                        "stack CPU and memory left out: not every node listed its containers"
                    );
                    Vec::new()
                }
            }
            Err(error) => {
                tracing::warn!(%error, "stack CPU and memory left out: the container list failed");
                Vec::new()
            }
        }
    };
    let (services, containers) = tokio::join!(services, containers);
    Ok(group_stacks(&services?, &containers))
}

/// The departed nodes, by the node list's rule; none when the manager or membership didn't answer.
async fn departed_nodes(
    database: &DatabaseConnection,
    manager: &dyn ManagerEngine,
    cancellation: &CancellationToken,
) -> HashSet<String> {
    let swarm = tokio::time::timeout(
        MANAGER_READ_DEADLINE,
        manager.list_swarm_nodes(cancellation),
    );
    match tokio::join!(list_nodes::execute(database), swarm) {
        (Ok((agents, _)), Ok(Ok(swarm))) => departed_agent_ids(&agents, &swarm),
        _ => HashSet::new(),
    }
}

/// Nodes whose listing failed, leaving out the departed ones.
fn failed_nodes(outcomes: &[NodeOutcome<()>], departed: &HashSet<String>) -> usize {
    outcomes
        .iter()
        .filter(|outcome| outcome.result.is_err() && !departed.contains(&outcome.node_id))
        .count()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{NodeOutcome, failed_nodes};
    use crate::nodes::fan_out::NodeFailure;

    #[test]
    fn a_departed_nodes_failure_does_not_blank_the_figures() {
        let outcome = |node_id: &str, result: Result<(), NodeFailure>| NodeOutcome {
            node_id: node_id.to_owned(),
            result,
        };
        let outcomes = vec![
            outcome("manager", Ok(())),
            outcome("replaced", Err(NodeFailure::Unavailable)),
        ];
        assert_eq!(failed_nodes(&outcomes, &HashSet::new()), 1);
        let departed = HashSet::from(["replaced".to_owned()]);
        assert_eq!(failed_nodes(&outcomes, &departed), 0);

        let mut live_failure = outcomes;
        live_failure.push(outcome("worker-1", Err(NodeFailure::Timeout)));
        assert_eq!(
            failed_nodes(&live_failure, &departed),
            1,
            "any other failure still blanks them"
        );
    }
}
