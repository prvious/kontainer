//! Mutation admission: every mutation is a POST carrying an `Idempotency-Key` that answers 202
//! `{ operationId, replayed }`. Later specs call [`NativeClient::admit`] for their routes.

use reqwest::{Method, header};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::client::{NativeClient, RetryPolicy, decode_json, parse_uuid};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;

/// One key per user gesture: 16–64 printable ASCII characters. A retried request reuses it, so
/// the controller replays instead of admitting a second operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    pub fn parse(value: &str) -> Result<Self, NativeClientError> {
        if (16..=64).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_graphic()) {
            Ok(Self(value.to_owned()))
        } else {
            Err(NativeClientError::InvalidRequest)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An admitted mutation to poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmittedOperation {
    pub operation_id: Uuid,
    /// The key already admitted this request; no second operation exists.
    pub replayed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AdmittedBody {
    operation_id: String,
    replayed: bool,
}

impl NativeClient {
    /// POST `body` to `path` with the key. Retried once on a classified pre-response transient
    /// failure: the key makes a repeat a replay, never a second mutation.
    ///
    /// # Errors
    ///
    /// [`NativeClientError::Conflict`] when the key was used for a different body,
    /// [`NativeClientError::InvalidRequest`] when the controller rejects the body or key.
    pub async fn admit(
        &self,
        access: &AccessCredential,
        path: &str,
        key: &IdempotencyKey,
        body: &impl Serialize,
        cancellation: &CancellationToken,
    ) -> Result<AdmittedOperation, NativeClientError> {
        let url = self.endpoint(path)?;
        let mut request = self.authorized_request(Method::POST, url, access)?;
        let payload =
            serde_json::to_vec(body).map_err(|_| NativeClientError::InvalidConfiguration)?;
        let key_value = header::HeaderValue::from_str(key.as_str())
            .map_err(|_| NativeClientError::InvalidRequest)?;
        request.headers_mut().insert("idempotency-key", key_value);
        request.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        *request.body_mut() = Some(payload.into());
        let response = self
            .execute(
                request,
                RetryPolicy::Once,
                self.limits.json_body_limit,
                cancellation,
            )
            .await?;
        let admitted = decode_json::<AdmittedBody>(&response)?;
        Ok(AdmittedOperation {
            operation_id: parse_uuid(&admitted.operation_id)?,
            replayed: admitted.replayed,
        })
    }
}
