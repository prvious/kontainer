use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::{
    services::domain::{ServiceRow, service_rows},
    swarm::engine::{ManagerEngine, ManagerError},
};

/// Manager reads answer within this, so a paused manager shows as unavailable quickly.
pub const MANAGER_READ_DEADLINE: Duration = Duration::from_secs(3);

/// Every service (or one stack's) with running counts from a single `tasks` call.
pub async fn execute(
    manager: &dyn ManagerEngine,
    stack: Option<&str>,
    cancellation: &CancellationToken,
) -> Result<Vec<ServiceRow>, ManagerError> {
    let read = async {
        let mut services = manager.list_services(cancellation).await?;
        if let Some(stack) = stack {
            services.retain(|service| service.stack.as_deref() == Some(stack));
        }
        if services.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<String> = services.iter().map(|service| service.id.clone()).collect();
        let tasks = manager.list_tasks(&ids, true, cancellation).await?;
        Ok(service_rows(services, &tasks))
    };
    tokio::time::timeout(MANAGER_READ_DEADLINE, read)
        .await
        .unwrap_or(Err(ManagerError::Unavailable))
}
