//! `POST /api/v1/auth/register`.

use reqwest::header;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use super::password_session::{PasswordSession, PasswordSessionReply};
use crate::client::{ErrorBodyPolicy, NativeClient, RetryPolicy, decode_json};
use crate::error::{NativeClientError, Traced};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RegisterOwnerBody<'a> {
    pub(crate) display_name: &'a str,
    pub(crate) email: &'a str,
    pub(crate) password: &'a str,
}

impl NativeClient {
    /// Create the owner on first run and sign in. Never retried: a replayed
    /// registration could race another first-run client.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::OwnerExists`] once registration has closed,
    /// [`NativeClientError::ValidationFailed`] with field problems, plus the
    /// shared transport, deadline, cancellation, and validation codes.
    pub async fn register_owner(
        &self,
        display_name: &str,
        email: &str,
        password: &str,
        cancellation: &CancellationToken,
    ) -> Result<PasswordSession, Traced<NativeClientError>> {
        let url = self
            .endpoint("/api/v1/auth/register")
            .map_err(Traced::untraced)?;
        let request = self
            .http
            .post(url)
            .header(header::ACCEPT, "application/json")
            .json(&RegisterOwnerBody {
                display_name,
                email,
                password,
            })
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
