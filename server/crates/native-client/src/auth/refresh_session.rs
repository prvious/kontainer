//! `POST /api/v1/auth/tokens/refresh`.

use reqwest::header;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use super::SessionCredentials;
use crate::client::{NativeClient, RetryPolicy, decode_json, parse_timestamp};
use crate::credentials::{AccessCredential, RefreshCredential};
use crate::error::NativeClientError;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RefreshSessionBody<'a> {
    pub(crate) refresh_credential: &'a str,
}

// No `Debug`: this holds plaintext credentials and must never be formatted.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RefreshSessionReply {
    access_credential: String,
    refresh_credential: String,
    access_expires_at: String,
    refresh_expires_at: String,
}

impl NativeClient {
    /// Rotate the refresh credential and return the successor pair.
    ///
    /// The request is single-flight by contract: retrying a consumed refresh
    /// credential revokes the whole session family, so this call is never
    /// retried.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::Unauthorized`] for an invalid, expired, revoked,
    /// or reused credential and [`NativeClientError::ServiceUnavailable`]
    /// when session persistence is unavailable, plus the shared transport,
    /// deadline, cancellation, and response-validation codes.
    pub async fn refresh_session(
        &self,
        refresh: &RefreshCredential,
        cancellation: &CancellationToken,
    ) -> Result<SessionCredentials, NativeClientError> {
        let url = self.endpoint("/api/v1/auth/tokens/refresh")?;
        let request = self
            .http
            .post(url)
            .header(header::ACCEPT, "application/json")
            .json(&RefreshSessionBody {
                refresh_credential: refresh.expose(),
            })
            .build()
            .map_err(|_| NativeClientError::InvalidConfiguration)?;
        let body = self
            .execute(
                request,
                RetryPolicy::Never,
                self.limits.json_body_limit,
                cancellation,
            )
            .await?;
        decode_json::<RefreshSessionReply>(&body)?.into_credentials()
    }
}

impl RefreshSessionReply {
    fn into_credentials(self) -> Result<SessionCredentials, NativeClientError> {
        Ok(SessionCredentials {
            access: AccessCredential::parse(&self.access_credential)
                .map_err(|_| NativeClientError::InvalidResponse)?,
            refresh: RefreshCredential::parse(&self.refresh_credential)
                .map_err(|_| NativeClientError::InvalidResponse)?,
            access_expires_at: parse_timestamp(&self.access_expires_at)?,
            refresh_expires_at: parse_timestamp(&self.refresh_expires_at)?,
        })
    }
}
