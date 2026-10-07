//! `GET` and `PATCH /api/v1/settings`.

use reqwest::{Method, header};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::client::{NativeClient, RetryPolicy, decode_json};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;

/// Retention the controller accepts, in days.
pub const RETENTION_DAYS: std::ops::RangeInclusive<u32> = 1..=365;

/// Controller-wide settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControllerSettings {
    pub metrics_retention_days: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SettingsBody {
    metrics_retention_days: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsPatchBody {
    metrics_retention_days: u32,
}

impl SettingsBody {
    fn into_settings(self) -> Result<ControllerSettings, NativeClientError> {
        if !RETENTION_DAYS.contains(&self.metrics_retention_days) {
            return Err(NativeClientError::InvalidResponse);
        }
        Ok(ControllerSettings {
            metrics_retention_days: self.metrics_retention_days,
        })
    }
}

impl NativeClient {
    pub async fn get_settings(
        &self,
        access: &AccessCredential,
        cancellation: &CancellationToken,
    ) -> Result<ControllerSettings, NativeClientError> {
        let url = self.endpoint("/api/v1/settings")?;
        let request = self.authorized_request(Method::GET, url, access)?;
        let body = self
            .execute(
                request,
                RetryPolicy::Once,
                self.limits.json_body_limit,
                cancellation,
            )
            .await?;
        decode_json::<SettingsBody>(&body)?.into_settings()
    }

    /// Change the metrics retention. Out-of-range values fail locally with
    /// [`NativeClientError::InvalidRequest`]; never retried.
    pub async fn update_settings(
        &self,
        access: &AccessCredential,
        metrics_retention_days: u32,
        cancellation: &CancellationToken,
    ) -> Result<ControllerSettings, NativeClientError> {
        if !RETENTION_DAYS.contains(&metrics_retention_days) {
            return Err(NativeClientError::InvalidRequest);
        }
        let url = self.endpoint("/api/v1/settings")?;
        let mut request = self.authorized_request(Method::PATCH, url, access)?;
        let payload = serde_json::to_vec(&SettingsPatchBody {
            metrics_retention_days,
        })
        .map_err(|_| NativeClientError::InvalidConfiguration)?;
        request.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        *request.body_mut() = Some(payload.into());
        let body = self
            .execute(
                request,
                RetryPolicy::Never,
                self.limits.json_body_limit,
                cancellation,
            )
            .await
            .map_err(|error| match error {
                NativeClientError::UnexpectedStatus { status: 422 } => {
                    NativeClientError::InvalidRequest
                }
                other => other,
            })?;
        decode_json::<SettingsBody>(&body)?.into_settings()
    }
}
