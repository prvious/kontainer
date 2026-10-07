//! `GET /api/v1/images`.

use reqwest::Method;
use tokio_util::sync::CancellationToken;

use super::{ImageList, ImageListBody};
use crate::client::{ErrorBodyPolicy, NativeClient, RetryPolicy, decode_json};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;

impl NativeClient {
    /// Every image on the answering member nodes, merged by id, with one outcome per member.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::AgentUnavailable`] when the controller has no agent RPC. A read-only
    /// GET, retried once on a classified pre-response transient failure.
    pub async fn list_images(
        &self,
        access: &AccessCredential,
        cancellation: &CancellationToken,
    ) -> Result<ImageList, NativeClientError> {
        let url = self.endpoint("/api/v1/images")?;
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
        decode_json::<ImageListBody>(&body)?.into_list()
    }
}
