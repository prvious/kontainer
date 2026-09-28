//! `service.scale`, `service.restart`, and `service.rollback`: apply the change on the manager
//! with the version the request was based on, then poll tasks until the service converges.
//! Accepted is not converged; after the deadline the operation fails `converge_timeout` with the
//! newest task error.

use std::time::Duration;

use async_trait::async_trait;
use kontainer_engine::ServiceMode;

use crate::{
    networks::operations::create::{manager_failure, not_a_manager},
    operations::{
        domain::{DurableOperation, OperationKind},
        executor::{ExecutionContext, ExecutionFailure, OperationExecutor},
    },
    services::domain::{
        Convergence, ServiceAction, ServiceActionPayload, latest_task_error, restart_convergence,
        rollback_convergence, scale_convergence,
    },
    swarm::engine::ManagerEngine,
};

const POLL_EVERY: Duration = Duration::from_secs(2);
const CONVERGE_WITHIN: Duration = Duration::from_secs(60);

pub struct ServiceConvergeExecutor {
    action: ServiceAction,
    poll: Duration,
    deadline: Duration,
}

impl ServiceConvergeExecutor {
    #[must_use]
    pub const fn new(action: ServiceAction) -> Self {
        Self {
            action,
            poll: POLL_EVERY,
            deadline: CONVERGE_WITHIN,
        }
    }

    /// The same executor polling faster, for tests.
    #[must_use]
    pub const fn with_timing(mut self, poll: Duration, deadline: Duration) -> Self {
        self.poll = poll;
        self.deadline = deadline;
        self
    }
}

fn progress(convergence: Convergence, phase: &str) -> serde_json::Value {
    serde_json::json!({
        "desired": convergence.desired,
        "running": convergence.running,
        "phase": phase,
    })
}

async fn apply(
    manager: &dyn ManagerEngine,
    action: ServiceAction,
    payload: &ServiceActionPayload,
    context: &ExecutionContext,
) -> Result<(), ExecutionFailure> {
    let result = match action {
        ServiceAction::Scale => {
            let replicas = payload
                .replicas
                .ok_or_else(ExecutionFailure::invalid_payload)?;
            manager
                .scale_service(
                    &payload.service_id,
                    replicas,
                    payload.version,
                    &context.cancellation,
                )
                .await
        }
        ServiceAction::Restart => {
            manager
                .force_update_service(&payload.service_id, payload.version, &context.cancellation)
                .await
        }
        ServiceAction::Rollback => {
            manager
                .rollback_service(&payload.service_id, payload.version, &context.cancellation)
                .await
        }
    };
    result.map_err(manager_failure)
}

#[async_trait]
impl OperationExecutor for ServiceConvergeExecutor {
    fn kind(&self) -> OperationKind {
        match self.action {
            ServiceAction::Scale => OperationKind::ServiceScale,
            ServiceAction::Restart => OperationKind::ServiceRestart,
            ServiceAction::Rollback => OperationKind::ServiceRollback,
        }
    }

    async fn execute(
        &self,
        operation: &DurableOperation,
        context: &ExecutionContext,
    ) -> Result<Option<serde_json::Value>, ExecutionFailure> {
        let payload: ServiceActionPayload = serde_json::from_value(operation.payload.clone())
            .map_err(|_| ExecutionFailure::invalid_payload())?;
        let manager = context.manager.as_ref().ok_or_else(not_a_manager)?.as_ref();
        let started = chrono::Utc::now().timestamp();
        apply(manager, self.action, &payload, context).await?;
        let mut last = Convergence {
            desired: payload.replicas.unwrap_or(0),
            running: 0,
            converged: false,
        };
        context.report_progress(progress(last, "accepted")).await;

        let deadline = tokio::time::Instant::now() + self.deadline;
        let mut last_error = None;
        loop {
            tokio::select! {
                () = context.cancellation.cancelled() => {
                    return Err(ExecutionFailure::new("cancelled", "The controller stopped before the service converged."));
                }
                () = tokio::time::sleep(self.poll) => {}
            }
            let services = manager.list_services(&context.cancellation).await;
            let tasks = manager
                .list_tasks(
                    std::slice::from_ref(&payload.service_id),
                    false,
                    &context.cancellation,
                )
                .await;
            if let (Ok(services), Ok(tasks)) = (services, tasks) {
                let Some(service) = services
                    .into_iter()
                    .find(|service| service.id == payload.service_id)
                else {
                    return Err(ExecutionFailure::new(
                        "not_found",
                        format!("{} no longer exists.", payload.name),
                    ));
                };
                let desired = match service.mode {
                    ServiceMode::Replicated { replicas } => replicas,
                    ServiceMode::Global => tasks
                        .iter()
                        .filter(|task| task.desired_state == "running")
                        .count() as u64,
                };
                last = match self.action {
                    ServiceAction::Scale => scale_convergence(&tasks, desired),
                    ServiceAction::Restart => restart_convergence(&tasks, desired, started),
                    ServiceAction::Rollback => {
                        rollback_convergence(&tasks, desired, service.update_state.as_deref())
                    }
                };
                last_error = latest_task_error(&tasks).or(last_error);
                if last.converged {
                    return Ok(Some(progress(last, "converged")));
                }
                context.report_progress(progress(last, "converging")).await;
            }
            if tokio::time::Instant::now() >= deadline {
                let reason = last_error
                    .map_or_else(String::new, |error| format!(" Latest task error: {error}."));
                return Err(ExecutionFailure::new(
                    "converge_timeout",
                    format!(
                        "{} didn't converge within {} s: {} of {} running.{reason}",
                        payload.name,
                        self.deadline.as_secs(),
                        last.running,
                        last.desired
                    ),
                )
                .with_detail(progress(last, "converging")));
            }
        }
    }
}
