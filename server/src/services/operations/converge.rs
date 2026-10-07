//! `service.scale`, `service.restart`, and `service.rollback`: apply the change on the manager
//! with the version the request was based on, then poll the service until it converges. Scaling
//! waits on task counts; restart and rollback wait on Swarm's update status (ADR 0034). Accepted
//! is not converged; after the deadline the operation fails `converge_timeout` with the newest
//! task error, or with the manager's failure when the last poll couldn't read the service.

use std::time::Duration;

use async_trait::async_trait;

use crate::{
    networks::operations::create::{manager_failure, not_a_manager},
    operations::{
        domain::{DurableOperation, OperationKind},
        executor::{ExecutionContext, ExecutionFailure, OperationExecutor},
    },
    services::{
        actions::get_service,
        domain::{
            ServiceAction, ServiceActionPayload, ServiceRow, scale_converged, update_verdict,
        },
    },
    swarm::engine::{ManagerEngine, ManagerError},
};

const POLL_EVERY: Duration = Duration::from_secs(2);
/// Rolling updates are paced by the service's own update settings, so they can take minutes.
pub(crate) const CONVERGE_WITHIN: Duration = Duration::from_secs(15 * 60);

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

fn progress(desired: u64, running: u64, phase: &str) -> serde_json::Value {
    serde_json::json!({
        "desired": desired,
        "running": running,
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
        apply(manager, self.action, &payload, context).await?;
        context
            .report_progress(progress(payload.replicas.unwrap_or(0), 0, "accepted"))
            .await;

        let deadline = tokio::time::Instant::now() + self.deadline;
        let mut last: Option<ServiceRow> = None;
        let mut last_task_error = None;
        loop {
            tokio::select! {
                () = context.cancellation.cancelled() => {
                    return Err(ExecutionFailure::new("cancelled", "The controller stopped before the service converged."));
                }
                () = tokio::time::sleep(self.poll) => {}
            }
            let poll_error = match get_service::execute(
                manager,
                &payload.service_id,
                &context.cancellation,
            )
            .await
            {
                Ok(Some(detail)) => {
                    last_task_error = detail.latest_error.or(last_task_error);
                    let row = detail.row;
                    let verdict = match self.action {
                        ServiceAction::Scale => {
                            scale_converged(&row, &detail.tasks).then_some(Ok(()))
                        }
                        ServiceAction::Restart | ServiceAction::Rollback => {
                            update_verdict(self.action, &row)
                        }
                    };
                    match verdict {
                        Some(Ok(())) => {
                            return Ok(Some(progress(row.desired, row.running, "converged")));
                        }
                        Some(Err((code, what))) => {
                            let reason = row
                                .service
                                .update_message
                                .as_deref()
                                .map_or_else(String::new, |message| format!(": {message}"));
                            return Err(ExecutionFailure::new(
                                code,
                                format!("Swarm {what} of {}{reason}.", payload.name),
                            )
                            .with_detail(progress(
                                row.desired,
                                row.running,
                                "converging",
                            )));
                        }
                        None => {}
                    }
                    context
                        .report_progress(progress(row.desired, row.running, "converging"))
                        .await;
                    last = Some(row);
                    None
                }
                Ok(None) | Err(ManagerError::NotFound) => {
                    return Err(ExecutionFailure::new(
                        "not_found",
                        format!("{} no longer exists.", payload.name),
                    ));
                }
                Err(error) => {
                    tracing::debug!(%error, service.id = %payload.service_id, "service convergence poll failed");
                    Some(error)
                }
            };
            if tokio::time::Instant::now() >= deadline {
                return Err(timed_out(
                    &payload.name,
                    self.deadline,
                    last.as_ref(),
                    poll_error,
                    last_task_error,
                ));
            }
        }
    }
}

/// The deadline passed: the manager's own failure when the last poll couldn't read the service,
/// else the counts and the newest task error.
fn timed_out(
    name: &str,
    deadline: Duration,
    last: Option<&ServiceRow>,
    last_poll_error: Option<ManagerError>,
    last_task_error: Option<String>,
) -> ExecutionFailure {
    if let Some(error) = last_poll_error {
        let failure = manager_failure(error);
        return ExecutionFailure::new(
            failure.code,
            format!(
                "{name} was changed, but the manager couldn't be read to see it converge. {}",
                failure.message
            ),
        );
    }
    let (desired, running) = last.map_or((0, 0), |row| (row.desired, row.running));
    let reason =
        last_task_error.map_or_else(String::new, |error| format!(" Latest task error: {error}."));
    ExecutionFailure::new(
        "converge_timeout",
        format!(
            "{name} didn't converge within {} s: {running} of {desired} running.{reason}",
            deadline.as_secs()
        ),
    )
    .with_detail(progress(desired, running, "converging"))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{CONVERGE_WITHIN, timed_out};
    use crate::{operations::domain::OperationKind, swarm::engine::ManagerError};

    #[test]
    fn the_operation_budget_outlasts_the_convergence_wait() {
        for kind in [
            OperationKind::ServiceScale,
            OperationKind::ServiceRestart,
            OperationKind::ServiceRollback,
        ] {
            assert!(kind.timeout() > CONVERGE_WITHIN, "{kind:?}");
        }
    }

    #[test]
    fn a_poll_error_at_the_deadline_is_reported_instead_of_counts() {
        let failure = timed_out(
            "webapp_web",
            Duration::from_secs(60),
            None,
            Some(ManagerError::QuorumUnavailable),
            None,
        );
        assert_eq!(failure.code, "quorum_unavailable");
        assert!(
            failure.message.starts_with("webapp_web was changed, but"),
            "{}",
            failure.message
        );
        assert!(!failure.message.contains("0 of 0"), "{}", failure.message);
    }
}
