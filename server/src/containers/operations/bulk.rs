//! `containers.bulk`: one lifecycle action across up to 200 containers on any nodes.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;

use super::lifecycle::{action_failure, node_client};
use crate::{
    containers::domain::BulkActionPayload,
    operations::{
        bulk::run_bulk,
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
