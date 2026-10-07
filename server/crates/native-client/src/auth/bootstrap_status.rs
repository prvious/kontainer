//! `GET /api/v1/auth/bootstrap`.

use reqwest::header;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::client::{ErrorBodyPolicy, NativeClient, RetryPolicy, decode_json};
use crate::error::{NativeClientError, Traced};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BootstrapReply {
    pub(crate) owner_exists: bool,
}

impl NativeClient {
    /// Whether the controller already has its owner, which decides between the
    /// sign-in and create-owner forms. Anonymous; never retried.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::ServiceUnavailable`] when persistence is down,
    /// plus the shared transport, deadline, cancellation, and validation codes.
    pub async fn bootstrap_status(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<bool, Traced<NativeClientError>> {
        let url = self
            .endpoint("/api/v1/auth/bootstrap")
            .map_err(Traced::untraced)?;
        let request = self
            .http
            .get(url)
            .header(header::ACCEPT, "application/json")
            .build()
            .map_err(|_| Traced::untraced(NativeClientError::InvalidConfiguration))?;
        let body = self
            .execute_traced(
                request,
                RetryPolicy::Never,
                self.limits.json_body_limit,
                cancellation,
                ErrorBodyPolicy::Account,
            )
            .await?;
        decode_json::<BootstrapReply>(&body)
            .map(|reply| reply.owner_exists)
            .map_err(Traced::untraced)
    }
}
