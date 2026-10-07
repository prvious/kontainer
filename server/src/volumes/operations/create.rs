//! `volume.create`: create one volume on one node.

use async_trait::async_trait;

use crate::{
    containers::operations::lifecycle::{action_failure, node_client},
    operations::{
        domain::{DurableOperation, OperationKind},
        executor::{ExecutionContext, ExecutionFailure, OperationExecutor},
    },
    rpc::{AgentClientError, AgentContainerAction, AgentVolumeCreate},
    volumes::domain::CreateVolumePayload,
};

pub struct CreateVolumeExecutor;

#[async_trait]
impl OperationExecutor for CreateVolumeExecutor {
    fn kind(&self) -> OperationKind {
        OperationKind::VolumeCreate
    }

    async fn execute(
        &self,
        operation: &DurableOperation,
        context: &ExecutionContext,
    ) -> Result<Option<serde_json::Value>, ExecutionFailure> {
        let payload: CreateVolumePayload = serde_json::from_value(operation.payload.clone())
            .map_err(|_| ExecutionFailure::invalid_payload())?;
        let client =
            node_client(&context.database, context.agents.as_ref(), &payload.node_id).await?;
        let request = AgentVolumeCreate {
            name: payload.name.clone(),
            driver: payload.driver,
            labels: payload.labels,
        };
        match client.create_volume(&request, &context.cancellation).await {
            Ok(volume) => Ok(Some(serde_json::json!({
                "nodeId": payload.node_id,
                "name": volume.name,
                "mountpoint": volume.mountpoint,
            }))),
            Err(AgentClientError::VolumeExists) => Err(ExecutionFailure::new(
                "volume_exists",
                format!(
                    "A volume named {} already exists on {}. Pick another name or another node.",
                    payload.name, payload.node_id
                ),
            )),
            Err(error) => Err(action_failure(error, AgentContainerAction::Start)),
        }
    }
}
