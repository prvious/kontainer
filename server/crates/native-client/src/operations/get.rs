//! `GET /api/v1/operations/{operation_id}`.

use reqwest::Method;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{OperationBody, OperationSnapshot};
use crate::client::{NativeClient, RetryPolicy, decode_json};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;

impl NativeClient {
    /// Poll one operation visible to the authenticated principal.
    ///
    /// The access credential is presented only in a sensitive `Authorization`
    /// header.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::NotFound`] when the operation does not exist or
    /// is not visible to the principal (the controller deliberately returns
    /// the same response for both), and
    /// [`NativeClientError::ServiceUnavailable`] while the controller is
    /// draining or session persistence is unavailable. This is a read-only
    /// GET, so a classified pre-response transient failure is retried exactly
    /// once inside the call's original deadline.
    pub async fn get_operation(
        &self,
        access: &AccessCredential,
        operation_id: Uuid,
        cancellation: &CancellationToken,
    ) -> Result<OperationSnapshot, NativeClientError> {
        let url = self.endpoint(&format!("/api/v1/operations/{operation_id}"))?;
        let request = self.authorized_request(Method::GET, url, access)?;
        let body = self
            .execute(
                request,
                RetryPolicy::Once,
                self.limits.json_body_limit,
                cancellation,
            )
            .await?;
        decode_json::<OperationBody>(&body)?.into_snapshot(operation_id)
    }
}
