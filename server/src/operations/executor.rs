//! The executor port: one implementation per operation kind, living with its domain.

use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{
    actions::report_progress,
    domain::{DurableOperation, OperationKind},
};
use crate::nodes::agents::AgentDirectory;

/// A failed execution: a stable code, a message fit for the desktop, and an optional detail
/// document (bulk results, for example). Never daemon, transport, or credential internals.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionFailure {
    pub code: String,
    pub message: String,
    pub detail: Option<serde_json::Value>,
}

impl ExecutionFailure {
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            detail: None,
        }
    }

    #[must_use]
    pub fn with_detail(mut self, detail: serde_json::Value) -> Self {
        self.detail = Some(detail);
        self
    }

    /// The payload did not match the kind's request type (admission validates first, so this
    /// means a stored payload from an older contract).
    #[must_use]
    pub fn invalid_payload() -> Self {
        Self::new(
            "invalid_payload",
            "The stored request is not valid for this operation.",
        )
    }
}

/// What an executor may use: the database, per-node agent clients, a cancellation token that
/// fires on timeout or forced shutdown, and progress reporting.
pub struct ExecutionContext {
    pub database: DatabaseConnection,
    pub agents: Option<Arc<dyn AgentDirectory>>,
    pub cancellation: CancellationToken,
    operation_id: Uuid,
}

impl ExecutionContext {
    #[must_use]
    pub fn new(
        database: DatabaseConnection,
        agents: Option<Arc<dyn AgentDirectory>>,
        cancellation: CancellationToken,
        operation_id: Uuid,
    ) -> Self {
        Self {
            database,
            agents,
            cancellation,
            operation_id,
        }
    }

    /// Record the kind's progress document. A failed write is logged, not fatal: progress is
    /// advisory and the terminal event still lands.
    pub async fn report_progress(&self, detail: serde_json::Value) {
        if let Err(error) =
            report_progress::execute(&self.database, self.operation_id, detail).await
        {
            tracing::warn!(%error, operation.id = %self.operation_id, "operation progress was not recorded");
        }
    }
}

#[async_trait]
pub trait OperationExecutor: Send + Sync {
    fn kind(&self) -> OperationKind;

    /// Run the operation; `Ok` carries the result detail shown on success.
    async fn execute(
        &self,
        operation: &DurableOperation,
        context: &ExecutionContext,
    ) -> Result<Option<serde_json::Value>, ExecutionFailure>;
}

/// Executors by kind, with per-kind timeouts (tests shorten them).
#[derive(Clone, Default)]
pub struct ExecutorRegistry {
    executors: HashMap<OperationKind, Arc<dyn OperationExecutor>>,
    timeouts: HashMap<OperationKind, Duration>,
}

impl ExecutorRegistry {
    #[must_use]
    pub fn with(mut self, executor: impl OperationExecutor + 'static) -> Self {
        self.executors.insert(executor.kind(), Arc::new(executor));
        self
    }

    #[must_use]
    pub fn with_timeout(mut self, kind: OperationKind, timeout: Duration) -> Self {
        self.timeouts.insert(kind, timeout);
        self
    }

    #[must_use]
    pub fn get(&self, kind: OperationKind) -> Option<Arc<dyn OperationExecutor>> {
        self.executors.get(&kind).cloned()
    }

    #[must_use]
    pub fn timeout(&self, kind: OperationKind) -> Duration {
        self.timeouts.get(&kind).copied().unwrap_or(kind.timeout())
    }
}

/// The registry every controller runs; later specs add their domains' executors here.
#[must_use]
pub fn production_registry() -> ExecutorRegistry {
    ExecutorRegistry::default().with(super::executors::noop::NoopExecutor)
}
