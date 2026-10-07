//! `POST /api/v1/operations/{operation_id}/cancel`.

use reqwest::Method;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{OperationBody, OperationSnapshot};
use crate::client::{NativeClient, RetryPolicy, decode_json};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;

impl NativeClient {
    /// Cancel an operation that has not started. Never retried.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::Conflict`] when it already started or finished (`not_cancellable`),
    /// [`NativeClientError::NotFound`] when it is not visible to the principal.
    pub async fn cancel_operation(
        &self,
        access: &AccessCredential,
        operation_id: Uuid,
        cancellation: &CancellationToken,
    ) -> Result<OperationSnapshot, NativeClientError> {
        let url = self.endpoint(&format!("/api/v1/operations/{operation_id}/cancel"))?;
        let request = self.authorized_request(Method::POST, url, access)?;
        let body = self
            .execute(
                request,
                RetryPolicy::Never,
                self.limits.json_body_limit,
                cancellation,
            )
            .await?;
        decode_json::<OperationBody>(&body)?.into_snapshot(operation_id)
    }
}
