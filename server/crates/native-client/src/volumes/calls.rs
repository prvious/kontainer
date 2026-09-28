//! `GET /api/v1/volumes`, `POST /api/v1/volumes`, and `DELETE /api/v1/volumes/{nodeId}/{name}`.

use std::collections::BTreeMap;

use reqwest::Method;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use super::{
    LOCAL_VOLUME_DRIVER, VolumeList, VolumeListBody, VolumeName, VolumeQuery, VolumeSpec,
    valid_labels,
};
use crate::client::{ErrorBodyPolicy, NativeClient, RetryPolicy, decode_json};
use crate::containers::NodeId;
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;
use crate::operations::{AdmittedOperation, IdempotencyKey};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateBody<'a> {
    node_id: &'a str,
    name: &'a str,
    driver: &'static str,
    labels: &'a BTreeMap<String, String>,
}

impl NativeClient {
    /// Volumes on the answering member nodes, one row per node and name, with one outcome per
    /// member. `query` narrows it to one node and/or one exact name.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::NotFound`] for an unknown node. A read-only GET, retried once on a
    /// classified pre-response transient failure.
    pub async fn list_volumes(
        &self,
        access: &AccessCredential,
        query: &VolumeQuery,
        cancellation: &CancellationToken,
    ) -> Result<VolumeList, NativeClientError> {
        let mut url = self.endpoint("/api/v1/volumes")?;
        if let Some(node) = &query.node {
            url.query_pairs_mut().append_pair("node", node.as_str());
        }
        if let Some(name) = &query.name {
            url.query_pairs_mut().append_pair("name", name.as_str());
        }
        let request = self.authorized_request(Method::GET, url, access)?;
        let body = self
            .execute_with_error_policy(
                request,
                RetryPolicy::Once,
                self.limits.collection_body_limit,
                cancellation,
                ErrorBodyPolicy::Inventory,
            )
            .await?;
        decode_json::<VolumeListBody>(&body)?.into_list()
    }

    /// Admit one `volume.create` operation. Invalid labels fail locally.
    pub async fn create_volume(
        &self,
        access: &AccessCredential,
        spec: &VolumeSpec,
        key: &IdempotencyKey,
        cancellation: &CancellationToken,
    ) -> Result<AdmittedOperation, NativeClientError> {
        if !valid_labels(&spec.labels) {
            return Err(NativeClientError::InvalidRequest);
        }
        let body = CreateBody {
            node_id: spec.node_id.as_str(),
            name: spec.name.as_str(),
            driver: LOCAL_VOLUME_DRIVER,
            labels: &spec.labels,
        };
        self.admit(access, "/api/v1/volumes", key, &body, cancellation)
            .await
    }

    /// Admit one `volume.remove` operation. A mounted volume fails it with `volume_in_use`.
    pub async fn remove_volume(
        &self,
        access: &AccessCredential,
        node_id: &NodeId,
        name: &VolumeName,
        key: &IdempotencyKey,
        cancellation: &CancellationToken,
    ) -> Result<AdmittedOperation, NativeClientError> {
        let path = format!("/api/v1/volumes/{}/{}", node_id.as_str(), name.as_str());
        self.admit_with(
            Method::DELETE,
            access,
            &path,
            key,
            &serde_json::json!({}),
            cancellation,
        )
        .await
    }
}
