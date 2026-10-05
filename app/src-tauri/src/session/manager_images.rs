//! Images across the cluster, their removal, and pulls, as credential-free IPC views.
//!
//! Mirrors `app/src/types/images.ts`; the shared fixture `app/tests/fixtures/images-ipc.json`
//! keeps both sides in sync.

use kontainer_native_client::{IdempotencyKey, ImageItem, ImageList, ImageNodes, NodeId};
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
pub(crate) struct ImageListView {
    items: Vec<ImageView>,
    nodes: Vec<NodeOutcomeView>,
    snapshot_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageView {
    image_id: String,
    repo_tags: Vec<String>,
    repo_digests: Vec<String>,
    size_bytes: u64,
    created_at: String,
    used_by: u32,
    node_ids: Vec<String>,
}

impl From<ImageItem> for ImageView {
    fn from(item: ImageItem) -> Self {
        Self {
            image_id: item.image_id,
            repo_tags: item.repo_tags,
            repo_digests: item.repo_digests,
            size_bytes: item.size_bytes,
            created_at: timestamp!(item.created_at),
            used_by: item.used_by,
            node_ids: item.node_ids.iter().map(ToString::to_string).collect(),
        }
    }
}

impl From<ImageList> for ImageListView {
    fn from(list: ImageList) -> Self {
        Self {
            items: list.items.into_iter().map(ImageView::from).collect(),
            nodes: list.nodes.into_iter().map(NodeOutcomeView::from).collect(),
            snapshot_at: timestamp!(list.snapshot_at),
        }
    }
}

/// Which nodes a removal targets, as React sends it: `"all"` or a list of node ids.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum ImageNodesInput {
    All(AllNodes),
    Only(Vec<String>),
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AllNodes {
    All,
}

fn node_ids(values: &[String]) -> Result<Vec<NodeId>, InventoryError> {
    values
        .iter()
        .map(|value| NodeId::parse(value).map_err(InventoryError::from))
        .collect()
}

impl SessionManager {
    pub(crate) async fn images(&self, profile_id: Uuid) -> Result<ImageListView, InventoryError> {
        let list = self
            .authenticated(profile_id, &(), |controller, access, cancellation, ()| {
                controller.list_images(access, cancellation)
            })
            .await?;
        Ok(list.into())
    }

    pub(crate) async fn remove_image(
        &self,
        profile_id: Uuid,
        image_id: &str,
        nodes: &ImageNodesInput,
        force: bool,
        idempotency_key: &str,
    ) -> Result<AdmittedView, InventoryError> {
        let nodes = match nodes {
            ImageNodesInput::All(AllNodes::All) => ImageNodes::All,
            ImageNodesInput::Only(values) => ImageNodes::Only(node_ids(values)?),
        };
        let arguments = (
            image_id.to_owned(),
            nodes,
            IdempotencyKey::parse(idempotency_key)?,
        );
        let admitted = self
            .authenticated(
                profile_id,
                &arguments,
                |controller, access, cancellation, (image_id, nodes, key)| {
                    controller.remove_image(access, image_id, nodes, force, key, cancellation)
                },
            )
            .await?;
        Ok(admitted.into())
    }

    pub(crate) async fn pull_image(
        &self,
        profile_id: Uuid,
        reference: &str,
        nodes: &[String],
        idempotency_key: &str,
    ) -> Result<AdmittedView, InventoryError> {
        let arguments = (
            reference.to_owned(),
            node_ids(nodes)?,
            IdempotencyKey::parse(idempotency_key)?,
        );
        let admitted = self
            .authenticated(
                profile_id,
                &arguments,
                |controller, access, cancellation, (reference, nodes, key)| {
                    controller.pull_image(access, reference, nodes, key, cancellation)
                },
            )
            .await?;
        Ok(admitted.into())
    }
}
