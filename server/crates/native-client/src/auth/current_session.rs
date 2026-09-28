//! `GET /api/v1/auth/session`.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::client::{ErrorBodyPolicy, NativeClient, RetryPolicy, decode_json, parse_uuid};
use crate::credentials::AccessCredential;
use crate::error::{NativeClientError, Traced};

/// The signed-in principal's non-secret identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// Immutable persisted principal identifier.
    pub principal_id: Uuid,
    /// Sign-in email, when the principal has one.
    pub email: Option<String>,
    /// Display name, when the principal has one.
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AccountReply {
    principal_id: String,
    email: Option<String>,
    display_name: Option<String>,
}

impl NativeClient {
    /// Read the signed-in principal's email and display name.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::Unauthorized`] for a rejected access credential,
    /// plus the shared transport, deadline, cancellation, and validation codes.
    pub async fn current_session(
        &self,
        access: &AccessCredential,
        cancellation: &CancellationToken,
    ) -> Result<Account, Traced<NativeClientError>> {
        let url = self
            .endpoint("/api/v1/auth/session")
            .map_err(Traced::untraced)?;
        let request = self
            .authorized_request(Method::GET, url, access)
            .map_err(Traced::untraced)?;
        let body = self
            .execute_traced(
                request,
                RetryPolicy::Never,
                self.limits.json_body_limit,
                cancellation,
                ErrorBodyPolicy::Ignore,
            )
            .await?;
        let reply = decode_json::<AccountReply>(&body).map_err(Traced::untraced)?;
        Ok(Account {
            principal_id: parse_uuid(&reply.principal_id).map_err(Traced::untraced)?,
            email: reply.email,
            display_name: reply.display_name,
        })
    }
}
