use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use loco_rs::{app::AppContext, controller::Routes};
use serde::Serialize;
use utoipa::ToSchema;

use crate::auth::{actions::current_account, domain::PrincipalContext, request};

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountResponse {
    principal_id: String,
    email: Option<String>,
    display_name: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/session",
    operation_id = "current_session",
    responses(
        (status = 200, description = "The signed-in principal's email and display name", body = AccountResponse),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 503, description = "Session persistence unavailable")
    )
)]
pub(crate) async fn current(
    State(context): State<AppContext>,
    principal: PrincipalContext,
) -> Response {
    match current_account::execute(&context.db, principal.principal_id()).await {
        Ok(Some(account)) => (
            StatusCode::OK,
            [(header::CACHE_CONTROL, "no-store")],
            Json(AccountResponse {
                principal_id: account.principal_id.to_string(),
                email: account.email,
                display_name: account.display_name,
            }),
        )
            .into_response(),
        // A live session whose principal vanished is not an identity to report.
        Ok(None) => request::unauthorized_bearer(),
        Err(_) => request::service_unavailable(),
    }
}

pub fn routes() -> Routes {
    Routes::new().get("/auth/session", current)
}
