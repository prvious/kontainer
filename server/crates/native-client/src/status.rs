//! `GET /api/v1/status`: the controller's build, log filter, and start time.

use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use crate::client::{NativeClient, RetryPolicy, decode_json};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;

const MAX_VERSION_LEN: usize = 64;
const MAX_LOG_LEVEL_LEN: usize = 512;

/// What the controller reports about itself for the sidebar and Settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerStatus {
    pub version: String,
    /// The tracing filter it logs with, such as `info` or a `RUST_LOG` value.
    pub log_level: String,
    pub started_at: DateTime<Utc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StatusBody {
    status: String,
    version: String,
    #[allow(dead_code)]
    architecture: String,
    log_level: String,
    started_at: DateTime<Utc>,
}

fn printable(value: &str, limit: usize) -> bool {
    !value.is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}

impl StatusBody {
    fn into_status(self) -> Result<ControllerStatus, NativeClientError> {
        if self.status != "ok"
            || !printable(&self.version, MAX_VERSION_LEN)
            || !printable(&self.log_level, MAX_LOG_LEVEL_LEN)
        {
            return Err(NativeClientError::InvalidResponse);
        }
        Ok(ControllerStatus {
            version: self.version,
            log_level: self.log_level,
            started_at: self.started_at,
        })
    }
}

impl NativeClient {
    pub async fn get_status(
        &self,
        access: &AccessCredential,
        cancellation: &CancellationToken,
    ) -> Result<ControllerStatus, NativeClientError> {
        let url = self.endpoint("/api/v1/status")?;
        let request = self.authorized_request(Method::GET, url, access)?;
        let body = self
            .execute(
                request,
                RetryPolicy::Once,
                self.limits.json_body_limit,
                cancellation,
            )
            .await?;
        decode_json::<StatusBody>(&body)?.into_status()
    }
}
