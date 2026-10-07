//! `network.remove`: a local network through its node's agent, a swarm-scope one on the manager
//! once no node reports endpoints on it. Attached containers fail the operation with `in_use`
//! and their names; Docker's own wording is never shown.

use async_trait::async_trait;

use crate::{
    containers::operations::lifecycle::{action_failure, node_client},
    networks::{
        actions::inspect_network::{self, InspectNetworkError},
        domain::{RemoveNetworkPayload, in_use_message},
        operations::create::{manager_failure, not_a_manager},
    },
    operations::{
        domain::{DurableOperation, OperationKind},
        executor::{ExecutionContext, ExecutionFailure, OperationExecutor},
    },
    rpc::{AgentClientError, AgentContainerAction, NetworkRemoval},
};

pub struct RemoveNetworkExecutor;

fn in_use(name: &str, containers: Vec<String>) -> ExecutionFailure {
    ExecutionFailure::new("in_use", in_use_message(name, &containers))
        .with_detail(serde_json::json!({ "inUseBy": containers }))
}

#[async_trait]
impl OperationExecutor for RemoveNetworkExecutor {
    fn kind(&self) -> OperationKind {
        OperationKind::NetworkRemove
    }

    async fn execute(
        &self,
        operation: &DurableOperation,
        context: &ExecutionContext,
    ) -> Result<Option<serde_json::Value>, ExecutionFailure> {
        let payload: RemoveNetworkPayload = serde_json::from_value(operation.payload.clone())
            .map_err(|_| ExecutionFailure::invalid_payload())?;
        let detail = serde_json::json!({ "networkId": payload.network_id, "name": payload.name, "nodeId": payload.node_id });
        if let Some(node_id) = payload.node_id.as_deref() {
            let client = node_client(&context.database, context.agents.as_ref(), node_id).await?;
            return match client
                .remove_network(&payload.network_id, &context.cancellation)
                .await
            {
                Ok(NetworkRemoval::Removed) => Ok(Some(detail)),
                Ok(NetworkRemoval::InUse(containers)) => Err(in_use(&payload.name, containers)),
                Err(AgentClientError::NotFound) => Err(ExecutionFailure::new(
                    "not_found",
                    format!("{} no longer exists on {node_id}.", payload.name),
                )),
                Err(error) => Err(action_failure(error, AgentContainerAction::Start)),
            };
        }
        let manager = context.manager.as_ref().ok_or_else(not_a_manager)?;
        let agents = context.agents.as_ref().ok_or_else(|| {
            ExecutionFailure::new(
                "agent_unavailable",
                "This controller has no agent RPC configured.",
            )
        })?;
        let attached = inspect_network::execute(
            &context.database,
            agents.as_ref(),
            &payload.network_id,
            None,
            &context.cancellation,
        )
        .await
        .map_err(|error| match error {
            InspectNetworkError::Persistence => {
                ExecutionFailure::new("persistence_unavailable", "Membership could not be read.")
            }
            InspectNetworkError::NotFound | InspectNetworkError::Agent(_) => ExecutionFailure::new(
                "agent_unavailable",
                "The nodes could not be asked about endpoints.",
            ),
        })?;
        if !attached.is_empty() {
            return Err(in_use(
                &payload.name,
                attached
                    .into_iter()
                    .map(|container| container.name)
                    .collect(),
            ));
        }
        manager
            .remove_network(&payload.network_id, &context.cancellation)
            .await
            .map_err(manager_failure)?;
        Ok(Some(detail))
    }
}
