//! `DELETE /api/v1/images/{imageId}` and `POST /api/v1/images/pulls`.

use reqwest::Method;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use super::{ImageNodes, MAX_IMAGE_REFERENCE_LEN, valid_image_id, valid_target_nodes};
use crate::client::NativeClient;
use crate::containers::NodeId;
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;
use crate::operations::{AdmittedOperation, IdempotencyKey};

#[derive(Serialize)]
#[serde(untagged)]
enum NodeIdsBody<'a> {
    All(&'static str),
    Only(Vec<&'a str>),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoveBody<'a> {
    node_ids: NodeIdsBody<'a>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    force: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PullBody<'a> {
    reference: &'a str,
    node_ids: Vec<&'a str>,
}

impl NativeClient {
    /// Admit one `image.remove` operation. An invalid id or node set fails locally.
    pub async fn remove_image(
        &self,
        access: &AccessCredential,
        image_id: &str,
        nodes: &ImageNodes,
        force: bool,
        key: &IdempotencyKey,
        cancellation: &CancellationToken,
    ) -> Result<AdmittedOperation, NativeClientError> {
        let node_ids = match nodes {
            ImageNodes::All => NodeIdsBody::All("all"),
            ImageNodes::Only(node_ids) if valid_target_nodes(node_ids) => {
                NodeIdsBody::Only(node_ids.iter().map(NodeId::as_str).collect())
            }
            ImageNodes::Only(_) => return Err(NativeClientError::InvalidRequest),
        };
        if !valid_image_id(image_id) {
            return Err(NativeClientError::InvalidRequest);
        }
        let path = format!("/api/v1/images/{image_id}");
        let body = RemoveBody { node_ids, force };
        self.admit_with(Method::DELETE, access, &path, key, &body, cancellation)
            .await
    }

    /// Admit one `image.pull` operation. The controller validates the reference grammar and
    /// answers [`NativeClientError::InvalidRequest`] for a malformed one; an empty or overlong
    /// reference or an invalid node set fails locally.
    pub async fn pull_image(
        &self,
        access: &AccessCredential,
        reference: &str,
        node_ids: &[NodeId],
        key: &IdempotencyKey,
        cancellation: &CancellationToken,
    ) -> Result<AdmittedOperation, NativeClientError> {
        if reference.is_empty()
            || reference.len() > MAX_IMAGE_REFERENCE_LEN
            || !reference.bytes().all(|byte| byte.is_ascii_graphic())
            || !valid_target_nodes(node_ids)
        {
            return Err(NativeClientError::InvalidRequest);
        }
        let body = PullBody {
            reference,
            node_ids: node_ids.iter().map(NodeId::as_str).collect(),
        };
        self.admit(access, "/api/v1/images/pulls", key, &body, cancellation)
            .await
    }
}
