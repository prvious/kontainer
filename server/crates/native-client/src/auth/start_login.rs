//! `POST /api/v1/auth/github/login-attempts`.

use reqwest::header;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use url::Url;

use super::LoginAttempt;
use crate::client::{NativeClient, RetryPolicy, decode_json, parse_timestamp};
use crate::credentials::LoginVerifier;
use crate::error::NativeClientError;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartLoginBody<'a> {
    pub(crate) code_challenge: &'a str,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StartLoginReply {
    attempt_id: String,
    authorize_url: String,
    expires_at: String,
}

impl NativeClient {
    /// Create a login attempt and return the GitHub authorization URL.
    ///
    /// The S256 challenge is derived from the caller's verifier; the verifier
    /// itself never crosses this endpoint and never enters a URL.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::InvalidRequest`] for a rejected challenge,
    /// [`NativeClientError::ServiceUnavailable`] when login is disabled or
    /// persistence is unavailable, plus the shared transport, deadline,
    /// cancellation, and response-validation codes. This call is never
    /// retried.
    pub async fn start_login(
        &self,
        verifier: &LoginVerifier,
        cancellation: &CancellationToken,
    ) -> Result<LoginAttempt, NativeClientError> {
        let url = self.endpoint("/api/v1/auth/github/login-attempts")?;
        let challenge = verifier.s256_challenge();
        let request = self
            .http
            .post(url)
            .header(header::ACCEPT, "application/json")
            .json(&StartLoginBody {
                code_challenge: &challenge,
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
        decode_json::<StartLoginReply>(&body)?.into_attempt()
    }
}

impl StartLoginReply {
    fn into_attempt(self) -> Result<LoginAttempt, NativeClientError> {
        Ok(LoginAttempt {
            attempt_id: crate::client::parse_uuid(&self.attempt_id)?,
            authorize_url: Url::parse(&self.authorize_url)
                .map_err(|_| NativeClientError::InvalidResponse)?,
            expires_at: parse_timestamp(&self.expires_at)?,
        })
    }
}
