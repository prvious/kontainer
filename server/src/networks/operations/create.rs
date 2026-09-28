//! `network.create`: an overlay on the manager, or a bridge on one node.

use async_trait::async_trait;
use kontainer_engine::NetworkCreate;

use crate::{
    containers::operations::lifecycle::{action_failure, node_client},
    networks::domain::CreateNetworkPayload,
    operations::{
        domain::{DurableOperation, OperationKind},
        executor::{ExecutionContext, ExecutionFailure, OperationExecutor},
    },
    rpc::{AgentContainerAction, AgentNetworkCreate},
    swarm::engine::ManagerError,
};

pub struct CreateNetworkExecutor;

pub(crate) fn not_a_manager() -> ExecutionFailure {
    ExecutionFailure::new(
        "not_a_manager",
        "The controller's Docker host is not a Swarm manager, so swarm networks can't be changed.",
    )
}

pub(crate) fn manager_failure(error: ManagerError) -> ExecutionFailure {
    match error {
        ManagerError::NotFound => {
            ExecutionFailure::new("not_found", "The network no longer exists.")
        }
        ManagerError::InvalidInput => {
            ExecutionFailure::new("invalid_request", "The manager refused the request.")
        }
        ManagerError::Rejected => {
            ExecutionFailure::new("engine_error", "Docker on the manager refused the change.")
        }
        ManagerError::Unavailable | ManagerError::Cancelled => {
            ExecutionFailure::new("manager_unavailable", "The Swarm manager did not answer.")
        }
    }
}

#[async_trait]
impl OperationExecutor for CreateNetworkExecutor {
    fn kind(&self) -> OperationKind {
        OperationKind::NetworkCreate
    }

    async fn execute(
        &self,
        operation: &DurableOperation,
        context: &ExecutionContext,
    ) -> Result<Option<serde_json::Value>, ExecutionFailure> {
        let payload: CreateNetworkPayload = serde_json::from_value(operation.payload.clone())
            .map_err(|_| ExecutionFailure::invalid_payload())?;
        let id = match (payload.driver.as_str(), payload.node_id.as_deref()) {
            ("overlay", None) => {
                let manager = context.manager.as_ref().ok_or_else(not_a_manager)?;
                let request = NetworkCreate {
                    name: payload.name.clone(),
                    driver: "overlay".to_owned(),
                    attachable: payload.attachable,
                    subnet: payload.subnet.clone(),
                    labels: std::collections::BTreeMap::new(),
                };
                manager
                    .create_network(&request, &context.cancellation)
                    .await
                    .map_err(manager_failure)?
            }
            ("bridge", Some(node_id)) => {
                let client =
                    node_client(&context.database, context.agents.as_ref(), node_id).await?;
                let request = AgentNetworkCreate {
                    name: payload.name.clone(),
                    subnet: payload.subnet.clone(),
                    labels: std::collections::BTreeMap::new(),
                };
                client
                    .create_network(&request, &context.cancellation)
                    .await
                    .map_err(|error| action_failure(error, AgentContainerAction::Start))?
            }
            _ => return Err(ExecutionFailure::invalid_payload()),
        };
        Ok(Some(serde_json::json!({
            "networkId": id,
            "name": payload.name,
            "nodeId": payload.node_id,
        })))
    }
}
