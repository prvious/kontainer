//! Node-scoped volumes, create, and remove, as credential-free IPC views.
//!
//! Mirrors `app/src/types/volumes.ts`; the shared fixture `app/tests/fixtures/volumes-ipc.json`
//! keeps both sides in sync.

use std::collections::BTreeMap;

use kontainer_native_client::{
    IdempotencyKey, NodeId, VolumeItem, VolumeList, VolumeName, VolumeQuery, VolumeSpec,
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
pub(crate) struct VolumeListView {
    items: Vec<VolumeView>,
    nodes: Vec<NodeOutcomeView>,
    snapshot_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VolumeView {
    node_id: String,
    name: String,
    driver: String,
    mount_point: String,
    created_at: Option<String>,
    labels: BTreeMap<String, String>,
    used_by: u32,
    users: Vec<String>,
    size_bytes: Option<u64>,
}

impl From<VolumeItem> for VolumeView {
    fn from(item: VolumeItem) -> Self {
        Self {
            node_id: item.node_id.to_string(),
            name: item.name,
            driver: item.driver,
            mount_point: item.mount_point,
            created_at: item.created_at.map(|at| timestamp!(at)),
            labels: item.labels,
            used_by: item.used_by,
            users: item.users,
            size_bytes: item.size_bytes,
        }
    }
}

impl From<VolumeList> for VolumeListView {
    fn from(list: VolumeList) -> Self {
        Self {
            items: list.items.into_iter().map(VolumeView::from).collect(),
            nodes: list.nodes.into_iter().map(NodeOutcomeView::from).collect(),
            snapshot_at: timestamp!(list.snapshot_at),
        }
    }
}

/// A create as React sends it; the driver is always `local`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CreateVolumeInput {
    pub node_id: String,
    pub name: String,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
}

impl SessionManager {
    pub(crate) async fn volumes(
        &self,
        profile_id: Uuid,
        node_id: Option<&str>,
        name: Option<&str>,
    ) -> Result<VolumeListView, InventoryError> {
        let query = VolumeQuery {
            node: node_id.map(NodeId::parse).transpose()?,
            name: name.map(VolumeName::parse).transpose()?,
        };
        let list = self
            .authenticated(
                profile_id,
                &query,
                |controller, access, cancellation, query| {
                    controller.list_volumes(access, query, cancellation)
                },
            )
            .await?;
        Ok(list.into())
    }

    pub(crate) async fn create_volume(
        &self,
        profile_id: Uuid,
        input: &CreateVolumeInput,
        idempotency_key: &str,
    ) -> Result<AdmittedView, InventoryError> {
        let arguments = (
            VolumeSpec {
                node_id: NodeId::parse(&input.node_id)?,
                name: VolumeName::parse(&input.name)?,
                labels: input.labels.clone(),
            },
            IdempotencyKey::parse(idempotency_key)?,
        );
        let admitted = self
            .authenticated(
                profile_id,
                &arguments,
                |controller, access, cancellation, (spec, key)| {
                    controller.create_volume(access, spec, key, cancellation)
                },
            )
            .await?;
        Ok(admitted.into())
    }

    pub(crate) async fn remove_volume(
        &self,
        profile_id: Uuid,
        node_id: &str,
        name: &str,
        idempotency_key: &str,
    ) -> Result<AdmittedView, InventoryError> {
        let arguments = (
            NodeId::parse(node_id)?,
            VolumeName::parse(name)?,
            IdempotencyKey::parse(idempotency_key)?,
        );
        let admitted = self
            .authenticated(
                profile_id,
                &arguments,
                |controller, access, cancellation, (node_id, name, key)| {
                    controller.remove_volume(access, node_id, name, key, cancellation)
                },
            )
            .await?;
        Ok(admitted.into())
    }
}
