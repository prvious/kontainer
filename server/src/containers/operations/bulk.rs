//! `containers.bulk`: one lifecycle action across up to 200 containers on any nodes.

use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use serde::Deserialize;

use super::lifecycle::{action_failure, node_client};
use crate::{
    containers::domain::{BulkActionPayload, stop_budget},
    operations::{
        bulk::{BULK_CONCURRENCY, run_bulk},
        domain::{DurableOperation, OperationKind},
        executor::{ExecutionContext, ExecutionFailure, OperationExecutor},
    },
    rpc::{AgentClient, AgentContainerAction},
};

pub struct ContainersBulkExecutor;

#[async_trait]
impl OperationExecutor for ContainersBulkExecutor {
    fn kind(&self) -> OperationKind {
        OperationKind::ContainersBulk
    }

    /// Targets run eight at a time, and each may wait out a stop's grace. A bulk request names
    /// no grace, so each wave gets Docker's default; the budget never drops below the kind's.
    fn timeout(&self, payload: &serde_json::Value) -> Duration {
        let targets =
            BulkActionPayload::deserialize(payload).map_or(0, |payload| payload.targets.len());
        let waves = u32::try_from(targets.div_ceil(BULK_CONCURRENCY)).unwrap_or(u32::MAX);
        self.kind()
            .timeout()
            .max(stop_budget(None).saturating_mul(waves))
    }

    async fn execute(
        &self,
        operation: &DurableOperation,
        context: &ExecutionContext,
    ) -> Result<Option<serde_json::Value>, ExecutionFailure> {
        let payload: BulkActionPayload = serde_json::from_value(operation.payload.clone())
            .map_err(|_| ExecutionFailure::invalid_payload())?;
        let action = AgentContainerAction::parse(&payload.action)
            .ok_or_else(ExecutionFailure::invalid_payload)?;
        // One client per node, resolved up front; a node that cannot be reached fails its targets.
        let mut clients: HashMap<String, Result<Arc<dyn AgentClient>, String>> = HashMap::new();
        for target in &payload.targets {
            if !clients.contains_key(&target.node_id) {
                let client =
                    node_client(&context.database, context.agents.as_ref(), &target.node_id)
                        .await
                        .map_err(|failure| failure.code);
                clients.insert(target.node_id.clone(), client);
            }
        }
        let clients = &clients;
        let force = payload.force;
        run_bulk(payload.targets, context, |target| async move {
            let client = clients
                .get(&target.node_id)
                .cloned()
                .unwrap_or_else(|| Err("node_not_found".to_owned()))?;
            client
                .container_action(
                    &target.container_id,
                    action,
                    force,
                    None,
                    &context.cancellation,
                )
                .await
                .map_err(|error| action_failure(error, action).code)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::ContainersBulkExecutor;
    use crate::operations::executor::OperationExecutor;

    #[test]
    fn the_budget_covers_every_wave_of_eight() {
        let budget = |targets: usize| {
            ContainersBulkExecutor.timeout(&serde_json::json!({
                "action": "stop",
                "targets": (0..targets)
                    .map(|index| serde_json::json!({"nodeId": "manager", "containerId": format!("{index:064}")}))
                    .collect::<Vec<_>>(),
            }))
        };
        // 50 targets are 7 waves of up to 10 s grace plus 10 s each.
        assert_eq!(budget(50), Duration::from_secs(140));
        assert_eq!(budget(200), Duration::from_secs(25 * 20));
        assert_eq!(budget(3), Duration::from_secs(60), "never below the kind's");
        assert_eq!(
            ContainersBulkExecutor.timeout(&serde_json::json!({})),
            Duration::from_secs(60)
        );
    }
}
