use sea_orm::DatabaseConnection;
use tokio_util::sync::CancellationToken;

use crate::{
    containers::actions::list_containers,
    nodes::agents::AgentDirectory,
    services::actions::list_services,
    stacks::domain::{StackRow, group_stacks},
    swarm::engine::{ManagerEngine, ManagerError},
};

/// Every stack from the manager's services, with CPU and memory from the containers fan-out.
/// Without agents, or when any node's container list fails, the figures are left out, not the
/// stacks: a partial sum would look complete.
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
        match list_containers::execute(database, directory, None, cancellation).await {
            Ok(list) => {
                let failed = list
                    .outcomes
                    .iter()
                    .filter(|outcome| outcome.result.is_err())
                    .count();
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
