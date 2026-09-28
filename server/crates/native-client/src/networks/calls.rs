//! `GET /api/v1/networks[/{networkId}]`, `POST /api/v1/networks`, and
//! `DELETE /api/v1/networks/{networkId}`.

use reqwest::Method;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use super::{
    AttachedContainer, NetworkContainersBody, NetworkId, NetworkList, NetworkListBody,
    NetworkPlacement, NetworkSpec, valid_network_name,
};
use crate::client::{ErrorBodyPolicy, NativeClient, RetryPolicy, decode_json};
use crate::containers::NodeId;
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;
use crate::operations::{AdmittedOperation, IdempotencyKey};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateBody<'a> {
    name: &'a str,
    driver: &'static str,
    attachable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    node_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subnet: Option<&'a str>,
}

impl NativeClient {
    /// Swarm-scope networks once, local ones per node, and whether the manager answered.
    pub async fn list_networks(
        &self,
        access: &AccessCredential,
        cancellation: &CancellationToken,
    ) -> Result<NetworkList, NativeClientError> {
        let url = self.endpoint("/api/v1/networks")?;
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
        decode_json::<NetworkListBody>(&body)?.into_list()
    }

    /// The containers attached to one network: on `node` for a local one, cluster-wide otherwise.
    pub async fn inspect_network(
        &self,
        access: &AccessCredential,
        network_id: &NetworkId,
        node: Option<&NodeId>,
        cancellation: &CancellationToken,
    ) -> Result<Vec<AttachedContainer>, NativeClientError> {
        let mut url = self.endpoint(&format!("/api/v1/networks/{}", network_id.as_str()))?;
        if let Some(node) = node {
            url.query_pairs_mut().append_pair("node", node.as_str());
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
        decode_json::<NetworkContainersBody>(&body)?.into_containers()
    }

    /// Admit one `network.create` operation. A bad name fails locally; the controller checks
    /// uniqueness and the subnet.
    pub async fn create_network(
        &self,
        access: &AccessCredential,
        spec: &NetworkSpec,
        key: &IdempotencyKey,
        cancellation: &CancellationToken,
    ) -> Result<AdmittedOperation, NativeClientError> {
        if !valid_network_name(&spec.name)
            || spec.subnet.as_ref().is_some_and(|subnet| subnet.len() > 64)
        {
            return Err(NativeClientError::InvalidRequest);
        }
        let body = match &spec.placement {
            NetworkPlacement::Overlay { attachable } => CreateBody {
                name: &spec.name,
                driver: "overlay",
                attachable: *attachable,
                node_id: None,
                subnet: spec.subnet.as_deref(),
            },
            NetworkPlacement::Bridge { node_id } => CreateBody {
                name: &spec.name,
                driver: "bridge",
                attachable: false,
                node_id: Some(node_id.as_str()),
                subnet: spec.subnet.as_deref(),
            },
        };
        self.admit(access, "/api/v1/networks", key, &body, cancellation)
            .await
    }

    /// Admit one `network.remove` operation: `node` for a local network, none for a swarm one.
    pub async fn remove_network(
        &self,
        access: &AccessCredential,
        network_id: &NetworkId,
        node: Option<&NodeId>,
        key: &IdempotencyKey,
        cancellation: &CancellationToken,
    ) -> Result<AdmittedOperation, NativeClientError> {
        let path = match node {
            Some(node) => format!(
                "/api/v1/networks/{}?node={}",
                network_id.as_str(),
                node.as_str()
            ),
            None => format!("/api/v1/networks/{}", network_id.as_str()),
        };
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
