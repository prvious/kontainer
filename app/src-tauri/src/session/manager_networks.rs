//! Networks across the cluster, their attached containers, create, and remove, as
//! credential-free IPC views.
//!
//! Mirrors `app/src/types/networks.ts`; the shared fixture `app/tests/fixtures/networks-ipc.json`
//! keeps both sides in sync.

use kontainer_native_client::{
    AttachedContainer, IdempotencyKey, ManagerStatus, NetworkId, NetworkItem, NetworkList,
    NetworkPlacement, NetworkScope, NetworkSpec, NodeId,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    containers::NodeOutcomeView, inventory::InventoryError, operations::AdmittedView,
    SessionManager,
};

/// The controller's canonical RFC 3339 millisecond form.
macro_rules! timestamp {
    ($value:expr) => {
        $value.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
    };
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetworkListView {
    items: Vec<NetworkView>,
    nodes: Vec<NodeOutcomeView>,
    manager: ManagerStatus,
    snapshot_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NetworkView {
    id: String,
    name: String,
    driver: String,
    scope: NetworkScope,
    subnets: Vec<String>,
    containers: u32,
    created_at: Option<String>,
    attachable: bool,
    builtin: bool,
    node_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachedContainerView {
    node_id: String,
    container_id: String,
    name: String,
}

impl From<NetworkItem> for NetworkView {
    fn from(item: NetworkItem) -> Self {
        Self {
            id: item.id,
            name: item.name,
            driver: item.driver,
            scope: item.scope,
            subnets: item.subnets,
            containers: item.containers,
            created_at: item.created_at.map(|at| timestamp!(at)),
            attachable: item.attachable,
            builtin: item.builtin,
            node_ids: item.node_ids.iter().map(ToString::to_string).collect(),
        }
    }
}

impl From<NetworkList> for NetworkListView {
    fn from(list: NetworkList) -> Self {
        Self {
            items: list.items.into_iter().map(NetworkView::from).collect(),
            nodes: list.nodes.into_iter().map(NodeOutcomeView::from).collect(),
            manager: list.manager,
            snapshot_at: timestamp!(list.snapshot_at),
        }
    }
}

impl From<AttachedContainer> for AttachedContainerView {
    fn from(container: AttachedContainer) -> Self {
        Self {
            node_id: container.node_id.to_string(),
            container_id: container.container_id,
            name: container.name,
        }
    }
}

/// A create as React sends it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CreateNetworkInput {
    pub name: String,
    /// `overlay` or `bridge`.
    pub driver: String,
    #[serde(default)]
    pub attachable: bool,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub subnet: Option<String>,
}

impl CreateNetworkInput {
    fn spec(&self) -> Result<NetworkSpec, InventoryError> {
        let placement = match (self.driver.as_str(), self.node_id.as_deref()) {
            ("overlay", None) => NetworkPlacement::Overlay {
                attachable: self.attachable,
            },
            ("bridge", Some(node_id)) => NetworkPlacement::Bridge {
                node_id: NodeId::parse(node_id)?,
            },
            _ => return Err(InventoryError::InvalidRequest),
        };
        Ok(NetworkSpec {
            name: self.name.clone(),
            placement,
            subnet: self.subnet.clone().filter(|subnet| !subnet.is_empty()),
        })
    }
}

impl SessionManager {
    pub(crate) async fn networks(
        &self,
        profile_id: Uuid,
    ) -> Result<NetworkListView, InventoryError> {
        let list = self
            .authenticated(profile_id, &(), |controller, access, cancellation, ()| {
                controller.list_networks(access, cancellation)
            })
            .await?;
        Ok(list.into())
    }

    pub(crate) async fn network_containers(
        &self,
        profile_id: Uuid,
        network_id: &str,
        node_id: Option<&str>,
    ) -> Result<Vec<AttachedContainerView>, InventoryError> {
        let arguments = (
            NetworkId::parse(network_id)?,
            node_id.map(NodeId::parse).transpose()?,
        );
        let containers = self
            .authenticated(
                profile_id,
                &arguments,
                |controller, access, cancellation, (network_id, node)| {
                    controller.inspect_network(access, network_id, node.as_ref(), cancellation)
                },
            )
            .await?;
        Ok(containers
            .into_iter()
            .map(AttachedContainerView::from)
            .collect())
    }

    pub(crate) async fn create_network(
        &self,
        profile_id: Uuid,
        input: &CreateNetworkInput,
        idempotency_key: &str,
    ) -> Result<AdmittedView, InventoryError> {
        let arguments = (input.spec()?, IdempotencyKey::parse(idempotency_key)?);
        let admitted = self
            .authenticated(
                profile_id,
                &arguments,
                |controller, access, cancellation, (spec, key)| {
                    controller.create_network(access, spec, key, cancellation)
                },
            )
            .await?;
        Ok(admitted.into())
    }

    pub(crate) async fn remove_network(
        &self,
        profile_id: Uuid,
        network_id: &str,
        node_id: Option<&str>,
        idempotency_key: &str,
    ) -> Result<AdmittedView, InventoryError> {
        let arguments = (
            NetworkId::parse(network_id)?,
            node_id.map(NodeId::parse).transpose()?,
            IdempotencyKey::parse(idempotency_key)?,
        );
        let admitted = self
            .authenticated(
                profile_id,
                &arguments,
                |controller, access, cancellation, (network_id, node, key)| {
                    controller.remove_network(access, network_id, node.as_ref(), key, cancellation)
                },
            )
            .await?;
        Ok(admitted.into())
    }
}
