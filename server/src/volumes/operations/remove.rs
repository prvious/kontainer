//! `volume.remove`: remove one volume from one node. Docker refuses a mounted volume even with
//! force, so that refusal fails the operation with the containers' names.

use async_trait::async_trait;

use crate::{
    containers::operations::lifecycle::{action_failure, node_client},
    operations::{
        domain::{DurableOperation, OperationKind},
        executor::{ExecutionContext, ExecutionFailure, OperationExecutor},
    },
    rpc::{AgentClientError, AgentContainerAction, VolumeRemoval},
    volumes::domain::{RemoveVolumePayload, in_use_message},
};

pub struct RemoveVolumeExecutor;

#[async_trait]
impl OperationExecutor for RemoveVolumeExecutor {
    fn kind(&self) -> OperationKind {
        OperationKind::VolumeRemove
    }

    async fn execute(
        &self,
        operation: &DurableOperation,
        context: &ExecutionContext,
    ) -> Result<Option<serde_json::Value>, ExecutionFailure> {
        let payload: RemoveVolumePayload = serde_json::from_value(operation.payload.clone())
            .map_err(|_| ExecutionFailure::invalid_payload())?;
        let client =
            node_client(&context.database, context.agents.as_ref(), &payload.node_id).await?;
        match client
            .remove_volume(&payload.name, &context.cancellation)
            .await
        {
            Ok(VolumeRemoval::Removed) => Ok(Some(serde_json::json!({
                "nodeId": payload.node_id,
                "name": payload.name,
            }))),
            Ok(VolumeRemoval::InUse(users)) => Err(ExecutionFailure::new(
                "volume_in_use",
                in_use_message(&users),
            )
            .with_detail(serde_json::json!({ "inUseBy": users }))),
            Err(AgentClientError::NotFound) => Err(ExecutionFailure::new(
                "not_found",
                format!(
                    "No volume named {} exists on {}.",
                    payload.name, payload.node_id
                ),
            )),
            Err(error) => Err(action_failure(error, AgentContainerAction::Start)),
        }
    }
}
