//! The in-process operation worker (ADR 0009): recover, then claim and execute with bounded
//! concurrency, and prune hourly, until shutdown.

use std::{panic::AssertUnwindSafe, sync::Arc, time::Duration};

use futures_util::FutureExt;
use sea_orm::DatabaseConnection;
use tokio::{sync::Semaphore, time::Instant};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use super::{
    actions::{claim_next, finalize, prune, recover_running},
    domain::{DurableOperation, OperationKind},
    executor::{ExecutionContext, ExecutorRegistry},
};
use crate::nodes::agents::AgentDirectory;

#[derive(Debug, Clone)]
pub struct WorkerSettings {
    /// Stable claim reference, recorded on each claimed operation.
    pub reference: String,
    pub concurrency: usize,
    pub idle_min: Duration,
    pub idle_max: Duration,
    pub maintenance_every: Duration,
    /// How long in-flight executions may finish after the drain begins before they are cancelled
    /// (they then stay `running` and recovery marks them `uncertain`).
    pub grace: Duration,
}

impl WorkerSettings {
    #[must_use]
    pub fn new(grace: Duration) -> Self {
        Self {
            reference: format!("controller:{}", std::process::id()),
            concurrency: 4,
            idle_min: Duration::from_millis(250),
            idle_max: Duration::from_secs(1),
            maintenance_every: Duration::from_secs(60 * 60),
            grace,
        }
    }
}

pub struct Worker {
    pub database: DatabaseConnection,
    pub registry: Arc<ExecutorRegistry>,
    pub agents: Option<Arc<dyn AgentDirectory>>,
    pub manager: Option<Arc<dyn crate::swarm::engine::ManagerEngine>>,
    pub settings: WorkerSettings,
}

impl Worker {
    /// Run until `draining`, then wait for in-flight executions within the grace period.
    pub async fn run(self, draining: CancellationToken) {
        if !self.recover(&draining).await {
            return;
        }
        let permits = Arc::new(Semaphore::new(self.settings.concurrency.max(1)));
        let executions = TaskTracker::new();
        let forced = CancellationToken::new();
        let mut idle = self.settings.idle_min;
        let mut next_maintenance = Instant::now();
        loop {
            if Instant::now() >= next_maintenance {
                self.maintain().await;
                next_maintenance = Instant::now() + self.settings.maintenance_every;
            }
            let permit = tokio::select! {
                biased;
                () = draining.cancelled() => break,
                permit = Arc::clone(&permits).acquire_owned() => match permit {
                    Ok(permit) => permit,
                    Err(_) => break,
                },
            };
            match claim_next::execute(&self.database, &self.settings.reference).await {
                Ok(Some(operation)) => {
                    idle = self.settings.idle_min;
                    let run = Execution {
                        database: self.database.clone(),
                        registry: Arc::clone(&self.registry),
                        agents: self.agents.clone(),
                        manager: self.manager.clone(),
                        forced: forced.clone(),
                    };
                    executions.spawn(async move {
                        run.execute(operation).await;
                        drop(permit);
                    });
                }
                Ok(None) => {
                    drop(permit);
                    tokio::select! {
                        () = draining.cancelled() => break,
                        () = tokio::time::sleep(idle) => {}
                    }
                    idle = (idle * 2).min(self.settings.idle_max);
                }
                Err(error) => {
                    drop(permit);
                    tracing::warn!(%error, "operation claim failed");
                    tokio::select! {
                        () = draining.cancelled() => break,
                        () = tokio::time::sleep(self.settings.idle_max) => {}
                    }
                }
            }
        }
        executions.close();
        if tokio::time::timeout(self.settings.grace, executions.wait())
            .await
            .is_err()
        {
            tracing::warn!(
                in_flight = executions.len(),
                "operations still running at shutdown are cancelled and will recover as uncertain"
            );
            forced.cancel();
            executions.wait().await;
        }
    }

    /// Mark whatever was running before this start `uncertain`, retrying until it lands or the
    /// controller drains. Returns false when draining won.
    async fn recover(&self, draining: &CancellationToken) -> bool {
        loop {
            match recover_running::execute(&self.database).await {
                Ok(count) => {
                    if count > 0 {
                        tracing::warn!(
                            count,
                            "operations running at the last shutdown are now uncertain"
                        );
                    }
                    return true;
                }
                Err(error) => {
                    tracing::warn!(%error, "operation recovery failed; retrying");
                    tokio::select! {
                        () = draining.cancelled() => return false,
                        () = tokio::time::sleep(self.settings.idle_max) => {}
                    }
                }
            }
        }
    }

    async fn maintain(&self) {
        match prune::execute(&self.database).await {
            Ok(report) if report.operations + report.idempotency_records > 0 => tracing::info!(
                operations = report.operations,
                idempotency_records = report.idempotency_records,
                "pruned terminal operations"
            ),
            Ok(_) => {}
            Err(error) => tracing::warn!(%error, "operation pruning failed"),
        }
    }
}

struct Execution {
    database: DatabaseConnection,
    registry: Arc<ExecutorRegistry>,
    agents: Option<Arc<dyn AgentDirectory>>,
    manager: Option<Arc<dyn crate::swarm::engine::ManagerEngine>>,
    forced: CancellationToken,
}

impl Execution {
    async fn execute(self, operation: DurableOperation) {
        let id = operation.id;
        let Some(outcome) = self.outcome(&operation).await else {
            // Forced shutdown: leave it `running` so recovery reports it as uncertain.
            return;
        };
        if let Err(error) = finalize::execute(&self.database, id, outcome).await {
            tracing::error!(%error, operation.id = %id, "operation outcome was not recorded");
        }
    }

    async fn outcome(&self, operation: &DurableOperation) -> Option<finalize::Outcome> {
        let Some(kind) = OperationKind::parse(&operation.kind) else {
            return Some(unsupported());
        };
        let Some(executor) = self.registry.get(kind) else {
            return Some(unsupported());
        };
        let timeout = self.registry.timeout(kind);
        let cancellation = self.forced.child_token();
        let context = ExecutionContext::new(
            self.database.clone(),
            self.agents.clone(),
            cancellation.clone(),
            operation.id,
        )
        .with_manager(self.manager.clone());
        let run = AssertUnwindSafe(executor.execute(operation, &context)).catch_unwind();
        let result = tokio::time::timeout(timeout, run).await;
        if self.forced.is_cancelled() {
            return None;
        }
        Some(match result {
            Ok(Ok(Ok(detail))) => finalize::Outcome::Succeeded { detail },
            Ok(Ok(Err(failure))) => finalize::Outcome::Failed {
                code: failure.code,
                message: failure.message,
                detail: failure.detail,
            },
            Ok(Err(_panic)) => {
                tracing::error!(operation.id = %operation.id, operation.kind = %operation.kind, "operation executor panicked");
                finalize::Outcome::Failed {
                    code: "internal_error".to_owned(),
                    message: "The operation stopped unexpectedly.".to_owned(),
                    detail: None,
                }
            }
            Err(_) => {
                cancellation.cancel();
                finalize::Outcome::Failed {
                    code: "timeout".to_owned(),
                    message: format!(
                        "The operation did not finish within {}s.",
                        timeout.as_secs()
                    ),
                    detail: None,
                }
            }
        })
    }
}

fn unsupported() -> finalize::Outcome {
    finalize::Outcome::Failed {
        code: "unsupported_kind".to_owned(),
        message: "This controller cannot run this kind of operation.".to_owned(),
        detail: None,
    }
}
