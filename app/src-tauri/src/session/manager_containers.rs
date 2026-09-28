//! Containers across the cluster and their actions, as credential-free IPC views.
//!
//! Mirrors `app/src/types/containers.ts`; the shared fixture `app/tests/fixtures/containers-ipc.json`
//! keeps both sides in sync.

use kontainer_native_client::{
    ContainerAction, ContainerActionOptions, ContainerItem, ContainerList, ContainerState,
    ContainerTarget, IdempotencyKey, NodeId, NodeOutcome, NodeOutcomeStatus,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{inventory::InventoryError, operations::AdmittedView, SessionManager};

/// The controller's canonical RFC 3339 millisecond form.
macro_rules! timestamp {
    ($value:expr) => {
        $value.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
    };
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainerListView {
    items: Vec<ContainerView>,
    nodes: Vec<NodeOutcomeView>,
    snapshot_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ContainerView {
    node_id: String,
    node_name: String,
    container_id: String,
    name: String,
    image: String,
    state: ContainerState,
    status: String,
    created_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    ports: Vec<String>,
    service_name: Option<String>,
    stack_namespace: Option<String>,
    task_id: Option<String>,
    cpu_percent: Option<f64>,
    memory_used_bytes: Option<u64>,
    memory_limit_bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct NodeOutcomeView {
    node_id: String,
    status: NodeOutcomeStatus,
    message: Option<String>,
}

impl From<ContainerItem> for ContainerView {
    fn from(item: ContainerItem) -> Self {
        Self {
            node_id: item.node_id.to_string(),
            node_name: item.node_name,
            container_id: item.container_id,
            name: item.name,
            image: item.image,
            state: item.state,
            status: item.status,
            created_at: timestamp!(item.created_at),
            started_at: item.started_at.map(|at| timestamp!(at)),
            finished_at: item.finished_at.map(|at| timestamp!(at)),
            ports: item.ports,
            service_name: item.service_name,
            stack_namespace: item.stack_namespace,
            task_id: item.task_id,
            cpu_percent: item.cpu_percent,
            memory_used_bytes: item.memory_used_bytes,
            memory_limit_bytes: item.memory_limit_bytes,
        }
    }
}

impl From<NodeOutcome> for NodeOutcomeView {
    fn from(outcome: NodeOutcome) -> Self {
        Self {
            node_id: outcome.node_id.to_string(),
            status: outcome.status,
            message: outcome.message,
        }
    }
}

impl From<ContainerList> for ContainerListView {
    fn from(list: ContainerList) -> Self {
        Self {
            items: list.items.into_iter().map(ContainerView::from).collect(),
            nodes: list.nodes.into_iter().map(NodeOutcomeView::from).collect(),
            snapshot_at: timestamp!(list.snapshot_at),
        }
    }
}

/// One container an action names, as React sends it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ContainerTargetInput {
    pub node_id: String,
    pub container_id: String,
}

/// `force` and `timeoutSeconds`, as React sends them.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ContainerActionOptionsInput {
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub timeout_seconds: Option<u32>,
}

fn target(input: &ContainerTargetInput) -> Result<ContainerTarget, InventoryError> {
    Ok(ContainerTarget {
        node_id: NodeId::parse(&input.node_id)?,
        container_id: input.container_id.clone(),
    })
}

impl SessionManager {
    pub(crate) async fn containers(
        &self,
        profile_id: Uuid,
        node_id: Option<&str>,
    ) -> Result<ContainerListView, InventoryError> {
        let node = node_id.map(NodeId::parse).transpose()?;
        let list = self
            .authenticated(
                profile_id,
                &node,
                |controller, access, cancellation, node| {
                    controller.list_containers(access, node.as_ref(), cancellation)
                },
            )
            .await?;
        Ok(list.into())
    }

    pub(crate) async fn container_action(
        &self,
        profile_id: Uuid,
        container: &ContainerTargetInput,
        action: &str,
        options: ContainerActionOptionsInput,
        idempotency_key: &str,
    ) -> Result<AdmittedView, InventoryError> {
        let arguments = (
            target(container)?,
            ContainerAction::parse(action)?,
            ContainerActionOptions {
                force: options.force,
                timeout_seconds: options.timeout_seconds,
            },
            IdempotencyKey::parse(idempotency_key)?,
        );
        let admitted = self
            .authenticated(
                profile_id,
                &arguments,
                |controller, access, cancellation, (target, action, options, key)| {
                    controller.container_action(
                        access,
                        target,
                        *action,
                        *options,
                        key,
                        cancellation,
                    )
                },
            )
            .await?;
        Ok(admitted.into())
    }

    pub(crate) async fn containers_bulk_action(
        &self,
        profile_id: Uuid,
        action: &str,
        targets: &[ContainerTargetInput],
        force: bool,
        idempotency_key: &str,
    ) -> Result<AdmittedView, InventoryError> {
        let arguments = (
            ContainerAction::parse(action)?,
            targets.iter().map(target).collect::<Result<Vec<_>, _>>()?,
            IdempotencyKey::parse(idempotency_key)?,
        );
        let admitted = self
            .authenticated(
                profile_id,
                &arguments,
                |controller, access, cancellation, (action, targets, key)| {
                    controller.containers_bulk_action(
                        access,
                        *action,
                        targets,
                        force,
                        key,
                        cancellation,
                    )
                },
            )
            .await?;
        Ok(admitted.into())
    }
}
