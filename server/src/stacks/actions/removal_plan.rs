use kontainer_engine::STACK_NAMESPACE_LABEL;
use tokio_util::sync::CancellationToken;

use crate::{
    services::actions::list_services::MANAGER_READ_DEADLINE,
    stacks::domain::RemovalPlan,
    swarm::engine::{ManagerEngine, ManagerError},
};

/// The stack's service ids with the plan, for the executor.
pub struct PlannedRemoval {
    pub service_ids: Vec<String>,
    pub network_ids: Vec<(String, String)>,
    pub plan: RemovalPlan,
}

/// What removing `stack` takes away: its services and its labelled networks, while the named
/// volumes its services mount are listed as retained. `Ok(None)` when no service carries the name.
pub async fn execute(
    manager: &dyn ManagerEngine,
    stack: &str,
    cancellation: &CancellationToken,
) -> Result<Option<PlannedRemoval>, ManagerError> {
    let read = async {
        let services: Vec<_> = manager
            .list_services(cancellation)
            .await?
            .into_iter()
            .filter(|service| service.stack.as_deref() == Some(stack))
            .collect();
        if services.is_empty() {
            return Ok(None);
        }
        let networks: Vec<_> = manager
            .list_networks(cancellation)
            .await?
            .into_iter()
            .filter(|network| {
                network
                    .labels
                    .get(STACK_NAMESPACE_LABEL)
                    .map(String::as_str)
                    == Some(stack)
            })
            .collect();
        let mut volumes: Vec<String> = services
            .iter()
            .flat_map(|service| service.volumes.iter().cloned())
            .collect();
        volumes.sort();
        volumes.dedup();
        let mut names: Vec<String> = services
            .iter()
            .map(|service| service.name.clone())
            .collect();
        names.sort();
        let mut network_names: Vec<String> = networks
            .iter()
            .map(|network| network.name.clone())
            .collect();
        network_names.sort();
        Ok(Some(PlannedRemoval {
            service_ids: services.into_iter().map(|service| service.id).collect(),
            network_ids: networks
                .into_iter()
                .map(|network| (network.id, network.name))
                .collect(),
            plan: RemovalPlan {
                services: names,
                networks: network_names,
                volumes_retained: volumes,
            },
        }))
    };
    tokio::time::timeout(MANAGER_READ_DEADLINE, read)
        .await
        .unwrap_or(Err(ManagerError::Unavailable))
}
