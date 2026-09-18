use axum::{
    Json,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::SecondsFormat;
use loco_rs::{app::AppContext, controller::Routes};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    auth::actions::{accept_github_callback, redeem_github_login, start_github_login},
    server::ControllerClusterId,
};

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StartLoginRequest {
    /// Desktop-generated PKCE S256 challenge. The verifier itself never
    /// crosses this endpoint; only the challenge is persisted.
    code_challenge: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartLoginResponse {
    attempt_id: String,
    authorize_url: String,
    expires_at: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RedeemLoginRequest {
    /// Desktop-held PKCE verifier matching the start challenge. JSON body
    /// only, never a URL.
    code_verifier: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubLoginResponse {
    cluster_id: String,
    principal_id: String,
    github_login: String,
    access_credential: String,
    refresh_credential: String,
    access_expires_at: String,
    refresh_expires_at: String,
}

#[derive(Deserialize)]
pub(crate) struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
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

fn service_unavailable_html() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::PRAGMA, "no-cache"),
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::REFERRER_POLICY, "no-referrer"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; frame-ancestors 'none'; base-uri 'none'",
            ),
        ],
        "Sign-in is temporarily unavailable. Return to Kontainer and try again.",
    )
        .into_response()
}

/// Generic browser completion page. Carries no credential, identity, or
/// provider detail; the desktop learns the outcome through redemption.
fn callback_html(success: bool) -> Response {
    let body = if success {
        "Sign-in approved. Return to Kontainer to finish signing in."
    } else {
        "Sign-in did not complete. Return to Kontainer and try again."
    };
    (
        if success {
            StatusCode::OK
        } else {
            StatusCode::BAD_REQUEST
        },
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::PRAGMA, "no-cache"),
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::REFERRER_POLICY, "no-referrer"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; frame-ancestors 'none'; base-uri 'none'",
            ),
        ],
        body,
    )
        .into_response()
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/github/login-attempts",
    request_body = StartLoginRequest,
    responses(
        (status = 201, description = "Login attempt with the GitHub authorization URL", body = StartLoginResponse),
        (status = 400, description = "Invalid PKCE challenge"),
        (status = 503, description = "GitHub login unavailable")
    ),
    security(())
)]
pub(crate) async fn start(
    State(context): State<AppContext>,
    Json(body): Json<StartLoginRequest>,
) -> Response {
    let Some(service) = context
        .shared_store
        .get::<crate::auth::github::GitHubAuthService>()
    else {
        return json_no_store(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorBody {
                error: "unavailable",
            },
        );
    };
    match start_github_login::execute(
        &context.db,
        &service,
        &context.config.server.host,
        &body.code_challenge,
    )
    .await
    {
        Ok(started) => json_no_store(
            StatusCode::CREATED,
            StartLoginResponse {
                attempt_id: started.attempt_id.to_string(),
                authorize_url: started.authorize_url,
                expires_at: started
                    .expires_at
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
            },
        ),
        Err(start_github_login::StartLoginError::InvalidChallenge) => json_no_store(
            StatusCode::BAD_REQUEST,
            ErrorBody {
                error: "invalid_request",
            },
        ),
        Err(
            start_github_login::StartLoginError::Disabled
            | start_github_login::StartLoginError::Persistence,
        ) => json_no_store(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorBody {
                error: "unavailable",
            },
        ),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/github/callback",
    params(
        ("code" = Option<String>, Query, description = "Provider authorization code, exchanged server-side only"),
        ("state" = Option<String>, Query, description = "Opaque single-use login state"),
        ("error" = Option<String>, Query, description = "Provider denial signal; never echoed")
    ),
    responses(
        (status = 200, description = "Browser step complete; redeem from the desktop"),
        (status = 400, description = "Login did not complete"),
        (status = 503, description = "GitHub login unavailable")
    ),
    security(())
)]
pub(crate) async fn callback(
    State(context): State<AppContext>,
    Query(query): Query<CallbackQuery>,
) -> Response {
    // Disabled login fails closed before touching attempt storage.
    let enabled = context
        .shared_store
        .get::<crate::auth::github::GitHubAuthService>()
        .is_some_and(|service| service.is_enabled());
    if !enabled {
        return service_unavailable_html();
    }
    let Some(state) = query.state.filter(|state| !state.is_empty()) else {
        return callback_html(false);
    };
    match accept_github_callback::execute(
        &context.db,
        &state,
        query.code.as_deref(),
        query.error.as_deref(),
    )
    .await
    {
        Ok(accept_github_callback::CallbackOutcome::Ready { .. }) => callback_html(true),
        Ok(accept_github_callback::CallbackOutcome::Denied) => callback_html(false),
        Err(_) => service_unavailable_html(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/github/login-attempts/{attempt_id}/redeem",
    params(("attempt_id" = String, Path, description = "Login attempt UUID")),
    request_body = RedeemLoginRequest,
    responses(
        (status = 200, description = "Session issued for the bootstrap Owner; credentials returned once", body = GithubLoginResponse),
        (status = 400, description = "Invalid attempt identifier"),
        (status = 403, description = "Login denied: unknown, replayed, expired, or non-Owner identity"),
        (status = 409, description = "Browser step not complete yet"),
        (status = 502, description = "GitHub exchange failed"),
        (status = 503, description = "GitHub login unavailable")
    ),
    security(())
)]
pub(crate) async fn redeem(
    State(context): State<AppContext>,
    Path(attempt_id): Path<String>,
    Json(body): Json<RedeemLoginRequest>,
) -> Response {
    let Ok(attempt_id) = Uuid::parse_str(&attempt_id) else {
        return json_no_store(
            StatusCode::BAD_REQUEST,
            ErrorBody {
                error: "invalid_request",
            },
        );
    };
    let (Some(service), Some(cluster)) = (
        context
            .shared_store
            .get::<crate::auth::github::GitHubAuthService>(),
        context.shared_store.get::<ControllerClusterId>(),
    ) else {
        return json_no_store(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorBody {
                error: "unavailable",
            },
        );
    };
    match redeem_github_login::execute(
        &context.db,
        &service,
        &context.config.server.host,
        attempt_id,
        &body.code_verifier,
    )
    .await
    {
        Ok(redeemed) => json_no_store(
            StatusCode::OK,
            GithubLoginResponse {
                cluster_id: cluster.0,
                principal_id: redeemed.principal_id.to_string(),
                github_login: redeemed.github_login,
                access_credential: redeemed.issued.access_plaintext,
                refresh_credential: redeemed.issued.refresh_plaintext,
                access_expires_at: redeemed
                    .issued
                    .access_expires_at
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
                refresh_expires_at: redeemed
                    .issued
                    .refresh_expires_at
                    .to_rfc3339_opts(SecondsFormat::Millis, true),
            },
        ),
        Err(redeem_github_login::RedeemError::NotReady) => {
            json_no_store(StatusCode::CONFLICT, ErrorBody { error: "not_ready" })
        }
        Err(redeem_github_login::RedeemError::Denied) => json_no_store(
            StatusCode::FORBIDDEN,
            ErrorBody {
                error: "login_denied",
            },
        ),
        Err(redeem_github_login::RedeemError::ProviderUnavailable) => json_no_store(
            StatusCode::BAD_GATEWAY,
            ErrorBody {
                error: "provider_unavailable",
            },
        ),
        Err(
            redeem_github_login::RedeemError::Disabled
            | redeem_github_login::RedeemError::Persistence,
        ) => json_no_store(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorBody {
                error: "unavailable",
            },
        ),
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .post("/auth/github/login-attempts", start)
        .get("/auth/github/callback", callback)
        .post("/auth/github/login-attempts/{attempt_id}/redeem", redeem)
}
