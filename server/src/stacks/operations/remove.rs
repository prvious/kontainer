//! `stack.remove`: remove every service of the stack, wait until their tasks are gone, then remove
//! the stack's networks (retrying while Swarm still detaches them). Volumes are never touched.

use std::time::Duration;

use async_trait::async_trait;

use crate::{
    networks::operations::create::{manager_failure, not_a_manager},
    operations::{
        domain::{DurableOperation, OperationKind},
        executor::{ExecutionContext, ExecutionFailure, OperationExecutor},
    },
    stacks::{actions::removal_plan, domain::StackRemovePayload},
    swarm::engine::ManagerError,
};

const POLL_EVERY: Duration = Duration::from_secs(2);
const WAIT_AT_MOST: Duration = Duration::from_secs(60);

pub struct RemoveStackExecutor {
    poll: Duration,
    deadline: Duration,
}

impl Default for RemoveStackExecutor {
    fn default() -> Self {
        Self {
            poll: POLL_EVERY,
            deadline: WAIT_AT_MOST,
        }
    }
}

impl RemoveStackExecutor {
    /// The same executor polling faster, for tests.
    #[must_use]
    pub const fn with_timing(poll: Duration, deadline: Duration) -> Self {
        Self { poll, deadline }
    }
}

fn detail(phase: &str, services: usize, networks: usize, volumes: &[String]) -> serde_json::Value {
    serde_json::json!({
        "phase": phase,
        "servicesRemoved": services,
        "networksRemoved": networks,
        "volumesRetained": volumes,
    })
}

#[async_trait]
impl OperationExecutor for RemoveStackExecutor {
    fn kind(&self) -> OperationKind {
        OperationKind::StackRemove
    }

    async fn execute(
        &self,
        operation: &DurableOperation,
        context: &ExecutionContext,
    ) -> Result<Option<serde_json::Value>, ExecutionFailure> {
        let payload: StackRemovePayload = serde_json::from_value(operation.payload.clone())
            .map_err(|_| ExecutionFailure::invalid_payload())?;
        let manager = context.manager.as_ref().ok_or_else(not_a_manager)?.as_ref();
        let Some(planned) = removal_plan::execute(manager, &payload.name, &context.cancellation)
            .await
            .map_err(manager_failure)?
        else {
            return Err(ExecutionFailure::new(
                "not_found",
                format!("No services carry the stack name {}.", payload.name),
            ));
        };
        let volumes = planned.plan.volumes_retained.clone();

        context
            .report_progress(detail("removing_services", 0, 0, &volumes))
            .await;
        let mut removed = 0;
        for service_id in &planned.service_ids {
            match manager
                .remove_service(service_id, &context.cancellation)
                .await
            {
                Ok(()) | Err(ManagerError::NotFound) => removed += 1,
                Err(error) => {
                    return Err(manager_failure(error).with_detail(detail(
                        "removing_services",
                        removed,
                        0,
                        &volumes,
                    )));
                }
            }
            context
                .report_progress(detail("removing_services", removed, 0, &volumes))
                .await;
        }

        // Tasks linger while their containers stop; networks can't go before they do.
        context
            .report_progress(detail("waiting_for_tasks", removed, 0, &volumes))
            .await;
        let deadline = tokio::time::Instant::now() + self.deadline;
        loop {
            match manager
                .list_tasks(&planned.service_ids, false, &context.cancellation)
                .await
            {
                // Docker answers 404 for a task filter naming only removed services.
                Ok(tasks) if tasks.is_empty() => break,
                Err(ManagerError::NotFound) => break,
                Ok(_) => {}
                Err(error) => return Err(manager_failure(error)),
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(ExecutionFailure::new(
                    "tasks_remaining",
                    format!("The services of {} were removed, but their tasks were still stopping after {} s. Its networks were left.", payload.name, self.deadline.as_secs()),
                )
                .with_detail(detail("waiting_for_tasks", removed, 0, &volumes)));
            }
            tokio::time::sleep(self.poll).await;
        }

        context
            .report_progress(detail("removing_networks", removed, 0, &volumes))
            .await;
        let mut networks_removed = 0;
        for (network_id, name) in &planned.network_ids {
            let deadline = tokio::time::Instant::now() + self.deadline;
            loop {
                match manager
                    .remove_network(network_id, &context.cancellation)
                    .await
                {
                    Ok(()) | Err(ManagerError::NotFound) => break,
                    // Swarm detaches the last endpoints a moment after the tasks go.
                    Err(ManagerError::Rejected) if tokio::time::Instant::now() < deadline => {
                        tokio::time::sleep(self.poll).await;
                    }
                    Err(ManagerError::Rejected) => {
                        return Err(ExecutionFailure::new(
                            "in_use",
                            format!("{name} was still in use after {} s; the stack's services are gone.", self.deadline.as_secs()),
                        )
                        .with_detail(detail("removing_networks", removed, networks_removed, &volumes)));
                    }
                    Err(error) => return Err(manager_failure(error)),
                }
            }
            networks_removed += 1;
            context
                .report_progress(detail(
                    "removing_networks",
                    removed,
                    networks_removed,
                    &volumes,
                ))
                .await;
        }
        Ok(Some(detail("removed", removed, networks_removed, &volumes)))
    }
}
