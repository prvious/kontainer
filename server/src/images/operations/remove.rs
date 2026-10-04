//! `image.remove`: remove one image from the listed nodes, or every member that has it.

use std::sync::Arc;

use async_trait::async_trait;
use futures_util::{StreamExt, stream};
use serde::Serialize;

use crate::{
    containers::operations::lifecycle::{action_failure, node_client},
    images::domain::{NodeSelection, RemoveImagePayload},
    nodes::actions::list_nodes,
    operations::{
        domain::{DurableOperation, OperationKind},
        executor::{ExecutionContext, ExecutionFailure, OperationExecutor},
    },
    rpc::{AgentClient, AgentClientError, AgentContainerAction},
};

const REMOVE_CONCURRENCY: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeRemoval {
    node_id: String,
    ok: bool,
    /// `all` reached a member without the image: nothing to remove there.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    skipped: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub struct RemoveImageExecutor;

#[async_trait]
impl OperationExecutor for RemoveImageExecutor {
    fn kind(&self) -> OperationKind {
        OperationKind::ImageRemove
    }

    async fn execute(
        &self,
        operation: &DurableOperation,
        context: &ExecutionContext,
    ) -> Result<Option<serde_json::Value>, ExecutionFailure> {
        let payload: RemoveImagePayload = serde_json::from_value(operation.payload.clone())
            .map_err(|_| ExecutionFailure::invalid_payload())?;
        let (node_ids, everywhere) = match payload.node_ids {
            NodeSelection::Nodes(node_ids) => (node_ids, false),
            NodeSelection::All(_) => {
                let (nodes, _) = list_nodes::execute(&context.database).await.map_err(|_| {
                    ExecutionFailure::new(
                        "persistence_unavailable",
                        "Membership could not be read.",
                    )
                })?;
                // Every member, offline ones included: an unreachable node fails its part, so
                // the image left there is reported rather than read as removed.
                (nodes.into_iter().map(|node| node.id).collect(), true)
            }
        };
        let image_id = payload.image_id.as_str();
        let force = payload.force;
        let results: Vec<NodeRemoval> = stream::iter(node_ids)
            .map(|node_id| async move {
                let client: Arc<dyn AgentClient> =
                    match node_client(&context.database, context.agents.as_ref(), &node_id).await {
                        Ok(client) => client,
                        Err(failure) => {
                            return NodeRemoval {
                                node_id,
                                ok: false,
                                skipped: false,
                                error: Some(failure.code),
                            };
                        }
                    };
                match client
                    .remove_image(image_id, force, &context.cancellation)
                    .await
                {
                    Ok(()) => NodeRemoval {
                        node_id,
                        ok: true,
                        skipped: false,
                        error: None,
                    },
                    Err(AgentClientError::NotFound) if everywhere => NodeRemoval {
                        node_id,
                        ok: true,
                        skipped: true,
                        error: None,
                    },
                    Err(error) => NodeRemoval {
                        node_id,
                        ok: false,
                        skipped: false,
                        error: Some(action_failure(error, AgentContainerAction::Remove).code),
                    },
                }
            })
            .buffer_unordered(REMOVE_CONCURRENCY)
            .collect()
            .await;
        let mut results = results;
        results.sort_by(|left, right| left.node_id.cmp(&right.node_id));
        let failed: Vec<_> = results.iter().filter(|result| !result.ok).collect();
        let detail = serde_json::json!({ "nodes": results });
        if failed.is_empty() {
            return Ok(Some(detail));
        }
        let codes: std::collections::BTreeSet<_> = failed
            .iter()
            .filter_map(|result| result.error.as_deref())
            .collect();
        let (code, message) = match codes.iter().next() {
            Some(&"image_in_use") if codes.len() == 1 => {
                ("image_in_use".to_owned(), in_use_message(force).to_owned())
            }
            _ => (
                "partial_failure".to_owned(),
                format!("{} of {} nodes failed.", failed.len(), results.len()),
            ),
        };
        Err(ExecutionFailure::new(code, message).with_detail(detail))
    }
}

/// Docker's 409: without force, a container or another tag holds the image and force overrides
/// that; with force, only a running container still does.
const fn in_use_message(force: bool) -> &'static str {
    if force {
        "A running container uses this image; stop it first."
    } else {
        "A container or another tag uses this image; delete it with force to remove it anyway."
    }
}

#[cfg(test)]
mod tests {
    use super::in_use_message;

    #[test]
    fn force_is_suggested_only_when_it_was_not_sent() {
        assert!(in_use_message(false).contains("with force"));
        assert_eq!(
            in_use_message(true),
            "A running container uses this image; stop it first."
        );
    }
}
