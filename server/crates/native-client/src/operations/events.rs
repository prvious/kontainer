//! `GET /api/v1/operations/{operation_id}/events`.

use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::OperationState;
use crate::client::{NativeClient, RetryPolicy, decode_json, parse_timestamp};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;

/// Most events one page carries.
pub const MAX_EVENT_PAGE: u32 = 100;

/// One entry of an operation's history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationEvent {
    pub sequence: i32,
    pub state: OperationState,
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub detail: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OperationEventsBody {
    items: Vec<OperationEventBody>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct OperationEventBody {
    sequence: i32,
    state: OperationState,
    error_code: Option<String>,
    message: Option<String>,
    detail: Option<serde_json::Value>,
    created_at: String,
}

impl OperationEventsBody {
    fn into_events(self, after: i32) -> Result<Vec<OperationEvent>, NativeClientError> {
        if self.items.len() > MAX_EVENT_PAGE as usize {
            return Err(NativeClientError::InvalidResponse);
        }
        let mut previous = after;
        let mut events = Vec::with_capacity(self.items.len());
        for item in self.items {
            // Strictly increasing and after the cursor, or the page is not the documented one.
            if item.sequence <= previous {
                return Err(NativeClientError::InvalidResponse);
            }
            previous = item.sequence;
            events.push(OperationEvent {
                sequence: item.sequence,
                state: item.state,
                error_code: item.error_code,
                message: item.message,
                detail: item.detail,
                created_at: parse_timestamp(&item.created_at)?,
            });
        }
        Ok(events)
    }
}

impl NativeClient {
    /// Events after `after`, oldest first, at most `limit` (1–100).
    pub async fn list_operation_events(
        &self,
        access: &AccessCredential,
        operation_id: Uuid,
        after: u32,
        limit: u32,
        cancellation: &CancellationToken,
    ) -> Result<Vec<OperationEvent>, NativeClientError> {
        let after = i32::try_from(after).map_err(|_| NativeClientError::InvalidRequest)?;
        if !(1..=MAX_EVENT_PAGE).contains(&limit) {
            return Err(NativeClientError::InvalidRequest);
        }
        let mut url = self.endpoint(&format!("/api/v1/operations/{operation_id}/events"))?;
        url.query_pairs_mut()
            .append_pair("after", &after.to_string())
            .append_pair("limit", &limit.to_string());
        let request = self.authorized_request(Method::GET, url, access)?;
        let body = self
            .execute(
                request,
                RetryPolicy::Once,
                self.limits.json_body_limit,
                cancellation,
            )
            .await?;
        decode_json::<OperationEventsBody>(&body)?.into_events(after)
    }
}
