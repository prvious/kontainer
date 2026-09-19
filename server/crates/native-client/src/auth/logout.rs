//! `DELETE /api/v1/auth/sessions/current`.

use reqwest::Method;
use tokio_util::sync::CancellationToken;

use crate::client::{NativeClient, RetryPolicy};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;

impl NativeClient {
    /// Revoke the session family behind an access credential.
    ///
    /// The credential is presented only in a sensitive `Authorization`
    /// header; it never enters the URL. Logout is never retried, because a
    /// repeated revocation is not a safe transparent operation for the
    /// caller's session bookkeeping.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::Unauthorized`] for a missing, malformed, expired,
    /// or revoked credential and [`NativeClientError::ServiceUnavailable`]
    /// when session persistence is unavailable, plus the shared transport,
    /// deadline, cancellation, and response-validation codes.
    pub async fn logout(
        &self,
        access: &AccessCredential,
        cancellation: &CancellationToken,
    ) -> Result<(), NativeClientError> {
        let url = self.endpoint("/api/v1/auth/sessions/current")?;
        let request = self.authorized_request(Method::DELETE, url, access)?;
        let body = self
            .execute(
                request,
                RetryPolicy::Never,
                self.limits.json_body_limit,
                cancellation,
            )
            .await?;
        if body.is_empty() {
            Ok(())
        } else {
            // The documented success is `204 No Content`; a success body is
            // contract drift, not an ignorable payload.
            Err(NativeClientError::InvalidResponse)
        }
    }
}
