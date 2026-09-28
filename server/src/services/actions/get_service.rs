use kontainer_engine::TaskSummary;
use tokio_util::sync::CancellationToken;

use crate::{
    services::{
        actions::list_services::MANAGER_READ_DEADLINE,
        domain::{ServiceRow, latest_task_error, service_rows},
    },
    swarm::engine::{ManagerEngine, ManagerError},
};

/// Tasks returned with one service, newest first.
const MAX_TASKS: usize = 50;

pub struct ServiceDetail {
    pub row: ServiceRow,
    pub tasks: Vec<TaskSummary>,
    pub latest_error: Option<String>,
}

/// One service with its recent tasks (every state) and the newest task error. `Ok(None)` when
/// the manager has no such service.
pub async fn execute(
    manager: &dyn ManagerEngine,
    service_id: &str,
    cancellation: &CancellationToken,
) -> Result<Option<ServiceDetail>, ManagerError> {
    let read = async {
        let services = manager.list_services(cancellation).await?;
        let Some(service) = services
            .into_iter()
            .find(|service| service.id == service_id)
        else {
            return Ok(None);
        };
        let mut tasks = manager
            .list_tasks(&[service_id.to_owned()], false, cancellation)
            .await?;
        tasks.sort_by_key(|task| std::cmp::Reverse(task.updated_at_seconds.unwrap_or(0)));
        let latest_error = latest_task_error(&tasks);
        let row = service_rows(vec![service], &tasks)
            .pop()
            .ok_or(ManagerError::Unavailable)?;
        tasks.truncate(MAX_TASKS);
        Ok(Some(ServiceDetail {
            row,
            tasks,
            latest_error,
        }))
    };
    tokio::time::timeout(MANAGER_READ_DEADLINE, read)
        .await
        .unwrap_or(Err(ManagerError::Unavailable))
}
