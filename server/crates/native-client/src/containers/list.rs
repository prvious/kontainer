//! `GET /api/v1/containers`.

use reqwest::Method;
use tokio_util::sync::CancellationToken;

use super::{ContainerList, ContainerListBody, NodeId};
use crate::client::{ErrorBodyPolicy, NativeClient, RetryPolicy, decode_json};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;

impl NativeClient {
    /// Every container on the answering member nodes (or on `node` alone), with one outcome per
    /// member so a silent node is visible rather than missing.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::TooManyContainers`] past the controller's cap (list one node then),
    /// [`NativeClientError::NotFound`] for an unknown `node`, and
    /// [`NativeClientError::AgentUnavailable`] when the controller has no agent RPC. A read-only
    /// GET, retried once on a classified pre-response transient failure.
    pub async fn list_containers(
        &self,
        access: &AccessCredential,
        node: Option<&NodeId>,
        cancellation: &CancellationToken,
    ) -> Result<ContainerList, NativeClientError> {
        let mut url = self.endpoint("/api/v1/containers")?;
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
        decode_json::<ContainerListBody>(&body)?.into_list()
    }
}
