//! `POST /api/v1/auth/password/login`.

use reqwest::header;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use super::password_session::{PasswordSession, PasswordSessionReply};
use crate::client::{ErrorBodyPolicy, NativeClient, RetryPolicy, decode_json};
use crate::error::{NativeClientError, Traced};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PasswordLoginBody<'a> {
    pub(crate) email: &'a str,
    pub(crate) password: &'a str,
}

impl NativeClient {
    /// Sign in with email and password. The password travels only in this
    /// JSON body; the call is never retried.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::InvalidCredentials`] with the attempts left,
    /// [`NativeClientError::LoginLocked`] with the seconds until unlock,
    /// [`NativeClientError::ValidationFailed`] with field problems, plus the
    /// shared transport, deadline, cancellation, and validation codes.
    pub async fn password_login(
        &self,
        email: &str,
        password: &str,
        cancellation: &CancellationToken,
    ) -> Result<PasswordSession, Traced<NativeClientError>> {
        let url = self
            .endpoint("/api/v1/auth/password/login")
            .map_err(Traced::untraced)?;
        let request = self
            .http
            .post(url)
            .header(header::ACCEPT, "application/json")
            .json(&PasswordLoginBody { email, password })
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
        decode_json::<PasswordSessionReply>(&body)
            .and_then(PasswordSessionReply::into_session)
            .map_err(Traced::untraced)
    }
}
