//! `GET /api/v1/nodes/{nodeId}/containers`.

use reqwest::Method;
use tokio_util::sync::CancellationToken;

use super::{ContainerPage, ContainerPageBody, ContainerPageRequest, NodeId};
use crate::client::{NativeClient, RetryPolicy, decode_json};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;

impl NativeClient {
    /// Read one bounded page of the configured node's containers.
    ///
    /// Local pagination validation runs before any network access. The access
    /// credential is presented only in a sensitive `Authorization` header; a
    /// page cursor is opaque, so pass [`ContainerPage::next_cursor`] back
    /// unchanged.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::InvalidPagination`] when the limit or cursor is
    /// outside the documented bounds (no request leaves the client),
    /// [`NativeClientError::NotFound`] for an unknown node,
    /// [`NativeClientError::ServiceUnavailable`] when the node agent is
    /// unavailable, and [`NativeClientError::GatewayTimeout`] when the agent
    /// deadline expires. This is a read-only GET, so a classified
    /// pre-response transient failure is retried exactly once inside the
    /// call's original deadline.
    pub async fn list_containers(
        &self,
        access: &AccessCredential,
        node_id: &NodeId,
        page: &ContainerPageRequest,
        cancellation: &CancellationToken,
    ) -> Result<ContainerPage, NativeClientError> {
        let (limit, cursor) = page.validated()?;
        let mut url = self.containers_endpoint(node_id)?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("limit", &limit.to_string());
            if let Some(cursor) = cursor {
                query.append_pair("cursor", cursor);
            }
        }
        let request = self.authorized_request(Method::GET, url, access)?;
        let body = self
            .execute(
                request,
                RetryPolicy::Once,
                self.limits.collection_body_limit,
                cancellation,
            )
            .await?;
        decode_json::<ContainerPageBody>(&body)?.into_page(node_id)
    }
}
