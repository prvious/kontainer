use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::SecondsFormat;
use loco_rs::{app::AppContext, controller::Routes};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::actions::{revoke_session_family, rotate_refresh, validate_access};

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RefreshRequest {
    /// Opaque refresh credential. Transported in the JSON body only, never
    /// in a URL, and returned only once at issuance or rotation.
    refresh_credential: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionTokenResponse {
    access_credential: String,
    refresh_credential: String,
    access_expires_at: String,
    refresh_expires_at: String,
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

fn json_no_store(status: StatusCode, value: impl Serialize) -> Response {
    (
        status,
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::PRAGMA, "no-cache"),
        ],
        Json(value),
    )
        .into_response()
}

fn unauthorized() -> Response {
    json_no_store(
        StatusCode::UNAUTHORIZED,
        ErrorBody {
            error: "unauthorized",
        },
    )
}

fn unauthorized_bearer() -> Response {
    let mut response = unauthorized();
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        header::HeaderValue::from_static("Bearer error=\"invalid_token\""),
    );
    response
}

fn service_unavailable() -> Response {
    json_no_store(
        StatusCode::SERVICE_UNAVAILABLE,
        ErrorBody {
            error: "unavailable",
        },
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/tokens/refresh",
    request_body = RefreshRequest,
    responses(
        (status = 200, description = "Rotated credential pair, returned once", body = SessionTokenResponse),
        (status = 401, description = "Invalid, expired, revoked, or reused refresh credential"),
        (status = 503, description = "Session persistence unavailable")
    )
)]
pub(crate) async fn refresh(
    State(context): State<AppContext>,
    Json(body): Json<RefreshRequest>,
) -> Response {
    match rotate_refresh::execute(&context.db, &body.refresh_credential).await {
        Ok(issued) => json_no_store(
            StatusCode::OK,
            SessionTokenResponse {
                access_credential: issued.access_plaintext,
                refresh_credential: issued.refresh_plaintext,
                access_expires_at: issued
                    .access_expires_at
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
                refresh_expires_at: issued
                    .refresh_expires_at
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
            },
        ),
        Err(rotate_refresh::RotateRefreshError::ReuseDetected)
        | Err(rotate_refresh::RotateRefreshError::Unauthorized) => unauthorized(),
        Err(rotate_refresh::RotateRefreshError::Persistence(_)) => service_unavailable(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/auth/sessions/current",
    responses(
        (status = 204, description = "Current session revoked"),
        (status = 401, description = "Missing, malformed, expired, or revoked access credential"),
        (status = 503, description = "Session persistence unavailable")
    )
)]
pub(crate) async fn logout_current(
    State(context): State<AppContext>,
    headers: HeaderMap,
) -> Response {
    let Some(presented) = bearer_credential(&headers) else {
        return unauthorized_bearer();
    };
    let session = match validate_access::execute(&context.db, presented).await {
        Ok(session) => session,
        Err(validate_access::ValidateAccessError::Unauthorized) => {
            return unauthorized_bearer();
        }
        Err(validate_access::ValidateAccessError::Persistence(_)) => {
            return service_unavailable();
        }
    };
    match revoke_session_family::execute(
        &context.db,
        session.family_id,
        crate::auth::domain::RevocationReason::Logout,
    )
    .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(revoke_session_family::RevokeSessionError::NotFound) => unauthorized_bearer(),
        Err(revoke_session_family::RevokeSessionError::Persistence(_)) => service_unavailable(),
    }
}

/// Exactly one `Authorization: Bearer <credential>` header value. Missing,
/// duplicated, wrong-scheme, query-parameter, and cookie credentials are
/// rejected without inspecting persistence.
fn bearer_credential(headers: &HeaderMap) -> Option<&str> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    value.strip_prefix("Bearer ")
}

pub fn routes() -> Routes {
    Routes::new()
        .post("/auth/tokens/refresh", refresh)
        .delete("/auth/sessions/current", logout_current)
}
