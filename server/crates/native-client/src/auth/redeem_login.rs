//! `POST /api/v1/auth/github/login-attempts/{attempt_id}/redeem`.

use reqwest::header;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{AuthenticatedSession, SessionCredentials};
use crate::client::{NativeClient, RetryPolicy, decode_json, parse_timestamp, parse_uuid};
use crate::credentials::{AccessCredential, LoginVerifier, RefreshCredential};
use crate::error::NativeClientError;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RedeemLoginBody<'a> {
    pub(crate) code_verifier: &'a str,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RedeemLoginReply {
    cluster_id: String,
    principal_id: String,
    github_login: String,
    access_credential: String,
    refresh_credential: String,
    access_expires_at: String,
    refresh_expires_at: String,
}

impl NativeClient {
    /// Exchange the desktop-held PKCE verifier for the bootstrap session.
    ///
    /// The verifier and the returned credentials travel only in JSON bodies;
    /// neither ever appears in a URL or a retry.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::InvalidRequest`] for a malformed attempt ID,
    /// [`NativeClientError::Forbidden`] when the identity is denied,
    /// [`NativeClientError::Conflict`] before the browser step completes,
    /// [`NativeClientError::ProviderUnavailable`] when GitHub could not be
    /// reached, and [`NativeClientError::ServiceUnavailable`] when login is
    /// disabled or persistence is unavailable. This call is never retried.
    pub async fn redeem_login(
        &self,
        attempt_id: Uuid,
        verifier: &LoginVerifier,
        cancellation: &CancellationToken,
    ) -> Result<AuthenticatedSession, NativeClientError> {
        let url = self.endpoint(&format!(
            "/api/v1/auth/github/login-attempts/{attempt_id}/redeem"
        ))?;
        let request = self
            .http
            .post(url)
            .header(header::ACCEPT, "application/json")
            .json(&RedeemLoginBody {
                code_verifier: verifier.expose(),
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
        decode_json::<RedeemLoginReply>(&body)?.into_session()
    }
}

impl RedeemLoginReply {
    fn into_session(self) -> Result<AuthenticatedSession, NativeClientError> {
        Ok(AuthenticatedSession {
            cluster_id: self.cluster_id,
            principal_id: parse_uuid(&self.principal_id)?,
            github_login: self.github_login,
            credentials: SessionCredentials {
                access: AccessCredential::parse(&self.access_credential)
                    .map_err(|_| NativeClientError::InvalidResponse)?,
                refresh: RefreshCredential::parse(&self.refresh_credential)
                    .map_err(|_| NativeClientError::InvalidResponse)?,
                access_expires_at: parse_timestamp(&self.access_expires_at)?,
                refresh_expires_at: parse_timestamp(&self.refresh_expires_at)?,
            },
        })
    }
}
