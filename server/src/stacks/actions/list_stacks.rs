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
/// Without agents (or when the container list fails) the figures are left out, not the stacks.
pub async fn execute(
    database: &DatabaseConnection,
    manager: &dyn ManagerEngine,
    directory: Option<&dyn AgentDirectory>,
    cancellation: &CancellationToken,
) -> Result<Vec<StackRow>, ManagerError> {
    let services = list_services::execute(manager, None, cancellation);
    let containers = async {
        match directory {
            Some(directory) => list_containers::execute(database, directory, None, cancellation)
                .await
                .map(|list| list.items)
                .unwrap_or_default(),
            None => Vec::new(),
        }
    };
    let (services, containers) = tokio::join!(services, containers);
    Ok(group_stacks(&services?, &containers))
}
