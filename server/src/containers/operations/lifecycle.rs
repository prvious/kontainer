//! `container.start` … `container.remove`: one lifecycle action on one container.

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::{DatabaseConnection, EntityTrait};

use crate::{
    containers::domain::{ContainerActionPayload, action_kind},
    nodes::{
        agents::{AgentDirectory, NodeEndpoint},
        entities::node,
    },
    operations::{
        domain::{DurableOperation, OperationKind},
        executor::{ExecutionContext, ExecutionFailure, OperationExecutor},
    },
    rpc::{AgentClient, AgentClientError, AgentContainerAction},
};

pub struct ContainerLifecycleExecutor(pub AgentContainerAction);

impl ContainerLifecycleExecutor {
    /// One executor per action, for the registry.
    pub fn all() -> impl Iterator<Item = Self> {
        AgentContainerAction::ALL.into_iter().map(Self)
    }
}

#[async_trait]
impl OperationExecutor for ContainerLifecycleExecutor {
    fn kind(&self) -> OperationKind {
        action_kind(self.0)
    }

    async fn execute(
        &self,
        operation: &DurableOperation,
        context: &ExecutionContext,
    ) -> Result<Option<serde_json::Value>, ExecutionFailure> {
        let payload: ContainerActionPayload = serde_json::from_value(operation.payload.clone())
            .map_err(|_| ExecutionFailure::invalid_payload())?;
        if AgentContainerAction::parse(&payload.action) != Some(self.0) {
            return Err(ExecutionFailure::invalid_payload());
        }
        let client =
            node_client(&context.database, context.agents.as_ref(), &payload.node_id).await?;
        client
            .container_action(
                &payload.container_id,
                self.0,
                payload.force,
                payload.timeout_seconds,
                &context.cancellation,
            )
            .await
            .map_err(|error| action_failure(error, self.0))?;
        Ok(Some(serde_json::json!({
            "nodeId": payload.node_id,
            "containerId": payload.container_id,
            "action": self.0.as_str(),
        })))
    }
}

/// The agent client for a member node, or a failure the operation can report.
pub(crate) async fn node_client(
    database: &DatabaseConnection,
    agents: Option<&Arc<dyn AgentDirectory>>,
    node_id: &str,
) -> Result<Arc<dyn AgentClient>, ExecutionFailure> {
    let agents = agents.ok_or_else(|| {
        ExecutionFailure::new(
            "agent_unavailable",
            "This controller has no agent RPC configured.",
        )
    })?;
    let member = node::Entity::find_by_id(node_id.to_owned())
        .one(database)
        .await
        .map_err(|_| {
            ExecutionFailure::new("persistence_unavailable", "Membership could not be read.")
        })?
        .ok_or_else(|| ExecutionFailure::new("node_not_found", "The node left the cluster."))?;
    agents
        .client(&NodeEndpoint {
            node_id: member.id,
            advertise_address: member.advertise_address,
        })
        .map_err(|error| action_failure(error, AgentContainerAction::Start))
}

/// Stable codes and plain messages; no transport or daemon detail.
pub(crate) fn action_failure(
    error: AgentClientError,
    action: AgentContainerAction,
) -> ExecutionFailure {
    let (code, message) = match error {
        AgentClientError::NotFound => (
            "not_found",
            "The container no longer exists on that node.".to_owned(),
        ),
        AgentClientError::AlreadyInState => (
            "already_in_state",
            format!("The container was already {}.", settled_state(action)),
        ),
        // Docker's 409: only a removal has a forced form to suggest.
        AgentClientError::ContainerRunning => (
            "container_running",
            match action {
                AgentContainerAction::Remove => {
                    "The container is running; remove it with force.".to_owned()
                }
                _ => format!(
                    "Docker refused to {} it in its current state.",
                    action.as_str()
                ),
            },
        ),
        AgentClientError::ImageInUse => (
            "image_in_use",
            "Containers use the image; delete it with force.".to_owned(),
        ),
        AgentClientError::BuiltinNetwork => (
            "builtin_network",
            "Built-in networks can't be removed.".to_owned(),
        ),
        AgentClientError::VolumeExists => (
            "volume_exists",
            "A volume with that name already exists on that node.".to_owned(),
        ),
        AgentClientError::EngineError => (
            "engine_error",
            "Docker on that node refused the action.".to_owned(),
        ),
        AgentClientError::TimedOut => (
            "agent_timeout",
            "The node agent did not answer in time.".to_owned(),
        ),
        AgentClientError::Unavailable | AgentClientError::Cancelled => (
            "agent_unavailable",
            "The node agent is unavailable.".to_owned(),
        ),
        AgentClientError::IdentityMismatch
        | AgentClientError::InvalidResponse
        | AgentClientError::InvalidPagination => (
            "invalid_response",
            "The node agent answered unexpectedly.".to_owned(),
        ),
    };
    ExecutionFailure::new(code, message)
}

const fn settled_state(action: AgentContainerAction) -> &'static str {
    match action {
        AgentContainerAction::Start
        | AgentContainerAction::Restart
        | AgentContainerAction::Unpause => "running",
        AgentContainerAction::Stop => "stopped",
        AgentContainerAction::Pause => "paused",
        AgentContainerAction::Remove => "removed",
    }
}

#[cfg(test)]
mod tests {
    use super::action_failure;
    use crate::rpc::{AgentClientError, AgentContainerAction};

    #[test]
    fn only_a_refused_removal_suggests_force() {
        let refused = |action| action_failure(AgentClientError::ContainerRunning, action);
        assert_eq!(
            refused(AgentContainerAction::Remove).message,
            "The container is running; remove it with force."
        );
        for (action, message) in [
            (
                AgentContainerAction::Pause,
                "Docker refused to pause it in its current state.",
            ),
            (
                AgentContainerAction::Unpause,
                "Docker refused to unpause it in its current state.",
            ),
            (
                AgentContainerAction::Start,
                "Docker refused to start it in its current state.",
            ),
        ] {
            let failure = refused(action);
            assert_eq!(
                (failure.code.as_str(), failure.message.as_str()),
                ("container_running", message)
            );
        }
    }
}
