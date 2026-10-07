//! `GET /api/v1/nodes`.

use reqwest::Method;
use tokio_util::sync::CancellationToken;

use super::{NodeList, NodeListBody};
use crate::client::{NativeClient, RetryPolicy, decode_json};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;

impl NativeClient {
    /// Every member node with its derived status and latest sample. A read-only GET, so a
    /// classified pre-response transient failure is retried once inside the deadline.
    pub async fn list_nodes(
        &self,
        access: &AccessCredential,
        cancellation: &CancellationToken,
    ) -> Result<NodeList, NativeClientError> {
        let url = self.endpoint("/api/v1/nodes")?;
        let request = self.authorized_request(Method::GET, url, access)?;
        let body = self
            .execute(
                request,
                RetryPolicy::Once,
                self.limits.collection_body_limit,
                cancellation,
            )
            .await?;
        decode_json::<NodeListBody>(&body)?.into_list()
    }
}
