//! Controller-wide settings. Spec 17 builds the page; the contract lives here.

use axum::{extract::State, http::StatusCode, response::Response};
use loco_rs::{app::AppContext, controller::Routes};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::nodes::{NodeErrorResponse, error, json_no_store};
use crate::{
    auth::domain::PrincipalContext,
    settings::actions::{
        ControllerSettings, get_settings,
        update_settings::{self, SettingsPatch, UpdateSettingsError},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SettingsResponse {
    /// Days of node samples kept, 1–365.
    pub metrics_retention_days: i32,
}

impl From<ControllerSettings> for SettingsResponse {
    fn from(settings: ControllerSettings) -> Self {
        Self {
            metrics_retention_days: settings.metrics_retention_days,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatchRequest {
    #[serde(default)]
    pub metrics_retention_days: Option<i32>,
}

#[utoipa::path(
    get,
    path = "/api/v1/settings",
    operation_id = "get_settings",
    responses(
        (status = 200, description = "Controller settings", body = SettingsResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 503, description = "Persistence unavailable or controller draining", body = NodeErrorResponse)
    )
)]
pub(crate) async fn show(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
) -> Response {
    match get_settings::execute(&context.db).await {
        Ok(settings) => json_no_store(StatusCode::OK, SettingsResponse::from(settings)),
        Err(_) => error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable"),
    }
}

#[utoipa::path(
    patch,
    path = "/api/v1/settings",
    operation_id = "update_settings",
    request_body = SettingsPatchRequest,
    responses(
        (status = 200, description = "Settings after the change", body = SettingsResponse),
        (status = 400, description = "Malformed body or unknown field", body = NodeErrorResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 422, description = "metricsRetentionDays outside 1–365", body = NodeErrorResponse),
        (status = 503, description = "Persistence unavailable or controller draining", body = NodeErrorResponse)
    )
)]
pub(crate) async fn update(
    State(context): State<AppContext>,
    _principal: PrincipalContext,
    body: axum::body::Bytes,
) -> Response {
    let Ok(request) = serde_json::from_slice::<SettingsPatchRequest>(&body) else {
        return error(StatusCode::BAD_REQUEST, "invalid_body");
    };
    let patch = SettingsPatch {
        metrics_retention_days: request.metrics_retention_days,
    };
    match update_settings::execute(&context.db, patch).await {
        Ok(settings) => json_no_store(StatusCode::OK, SettingsResponse::from(settings)),
        Err(UpdateSettingsError::RetentionOutOfRange) => {
            error(StatusCode::UNPROCESSABLE_ENTITY, "retention_out_of_range")
        }
        Err(UpdateSettingsError::Persistence(_)) => {
            error(StatusCode::SERVICE_UNAVAILABLE, "persistence_unavailable")
        }
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .get("/settings", show)
        .patch("/settings", update)
}
