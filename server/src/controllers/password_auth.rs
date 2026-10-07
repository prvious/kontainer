//! Owner registration and email/password sign-in (ADR 0001). Lifecycle
//! routes: no access-session middleware; they issue the session.

use axum::{
    Json,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::SecondsFormat;
use loco_rs::{app::AppContext, controller::Routes};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    auth::{
        actions::{bootstrap_status, password_login, register_owner},
        credentials::SetupCode,
        domain::IssuedSession,
        throttle::LoginThrottle,
        validation::FieldErrors,
    },
    server::ControllerClusterId,
};

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BootstrapResponse {
    owner_exists: bool,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RegisterRequest {
    display_name: String,
    email: String,
    /// 12–128 characters. JSON body only; never logged or echoed.
    password: String,
    /// The code the controller printed at startup (ADR 0029). Never logged or echoed.
    setup_code: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PasswordLoginRequest {
    email: String,
    password: String,
}

/// Issued exactly once per sign-in, with `Cache-Control: no-store`.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PasswordSessionResponse {
    cluster_id: String,
    principal_id: String,
    email: String,
    display_name: String,
    access_credential: String,
    refresh_credential: String,
    access_expires_at: String,
    refresh_expires_at: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ValidationErrorResponse {
    /// Always `validation_failed`.
    error: &'static str,
    #[schema(value_type = Object)]
    fields: FieldErrors,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InvalidCredentialsResponse {
    /// Always `invalid_credentials`, for an unknown email and a wrong password alike.
    error: &'static str,
    attempts_remaining: u32,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LockedResponse {
    /// Always `locked`.
    error: &'static str,
    retry_after_seconds: u32,
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

fn unavailable() -> Response {
    json_no_store(
        StatusCode::SERVICE_UNAVAILABLE,
        ErrorBody {
            error: "unavailable",
        },
    )
}

/// Every Argon2id permit is in use; the attempt was not counted.
fn busy() -> Response {
    json_no_store(StatusCode::TOO_MANY_REQUESTS, ErrorBody { error: "busy" })
}

fn validation_failed(fields: FieldErrors) -> Response {
    json_no_store(
        StatusCode::BAD_REQUEST,
        ValidationErrorResponse {
            error: "validation_failed",
            fields,
        },
    )
}

fn session_response(
    status: StatusCode,
    cluster: ControllerClusterId,
    issued: IssuedSession,
    email: String,
    display_name: String,
) -> Response {
    json_no_store(
        status,
        PasswordSessionResponse {
            cluster_id: cluster.0,
            principal_id: issued.principal_id.to_string(),
            email,
            display_name,
            access_credential: issued.access_plaintext,
            refresh_credential: issued.refresh_plaintext,
            access_expires_at: issued
                .access_expires_at
                .to_rfc3339_opts(SecondsFormat::Millis, true),
            refresh_expires_at: issued
                .refresh_expires_at
                .to_rfc3339_opts(SecondsFormat::Millis, true),
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/bootstrap",
    responses(
        (status = 200, description = "Whether the owner account exists; reveals nothing beyond setup being done", body = BootstrapResponse),
        (status = 503, description = "Persistence unavailable")
    ),
    security(())
)]
pub(crate) async fn bootstrap(State(context): State<AppContext>) -> Response {
    match bootstrap_status::execute(&context.db).await {
        Ok(owner_exists) => json_no_store(StatusCode::OK, BootstrapResponse { owner_exists }),
        Err(_) => unavailable(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "Owner created and signed in; credentials returned once", body = PasswordSessionResponse),
        (status = 400, description = "Field errors (`validation_failed`); a missing or wrong setup code is `fields.setupCode`", body = ValidationErrorResponse),
        (status = 409, description = "An owner already exists (`owner_exists`); registration is closed"),
        (status = 429, description = "Too many password checks are running (`busy`); try again"),
        (status = 503, description = "Persistence unavailable")
    ),
    security(())
)]
pub(crate) async fn register(
    State(context): State<AppContext>,
    Json(body): Json<RegisterRequest>,
) -> Response {
    let Some(cluster) = context.shared_store.get::<ControllerClusterId>() else {
        return unavailable();
    };
    let expected_code = context.shared_store.get::<SetupCode>();
    let registration = register_owner::OwnerRegistration {
        display_name: body.display_name,
        email: body.email,
        password: body.password,
        setup_code: body.setup_code,
    };
    match register_owner::execute(&context.db, registration, expected_code.as_ref()).await {
        Ok(owner) => session_response(
            StatusCode::CREATED,
            cluster,
            owner.issued,
            owner.email,
            owner.display_name,
        ),
        Err(register_owner::RegisterOwnerError::Invalid(fields)) => validation_failed(fields),
        Err(register_owner::RegisterOwnerError::OwnerExists) => json_no_store(
            StatusCode::CONFLICT,
            ErrorBody {
                error: "owner_exists",
            },
        ),
        Err(register_owner::RegisterOwnerError::Busy) => busy(),
        Err(register_owner::RegisterOwnerError::Persistence) => unavailable(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/password/login",
    request_body = PasswordLoginRequest,
    responses(
        (status = 200, description = "Signed in; credentials returned once", body = PasswordSessionResponse),
        (status = 400, description = "Field errors (`validation_failed`)", body = ValidationErrorResponse),
        (status = 401, description = "Email or password is incorrect", body = InvalidCredentialsResponse),
        (status = 429, description = "Ten failures locked this email for fifteen minutes (`locked`), or too many password checks are running (`busy`, not counted)", body = LockedResponse),
        (status = 503, description = "Persistence unavailable")
    ),
    security(())
)]
pub(crate) async fn password_login(
    State(context): State<AppContext>,
    Json(body): Json<PasswordLoginRequest>,
) -> Response {
    let (Some(cluster), Some(throttle)) = (
        context.shared_store.get::<ControllerClusterId>(),
        context.shared_store.get::<LoginThrottle>(),
    ) else {
        return unavailable();
    };
    match password_login::execute(&context.db, &throttle, &body.email, &body.password).await {
        Ok(session) => session_response(
            StatusCode::OK,
            cluster,
            session.issued,
            session.email,
            session.display_name,
        ),
        Err(password_login::PasswordLoginError::Invalid(fields)) => validation_failed(fields),
        Err(password_login::PasswordLoginError::InvalidCredentials { attempts_remaining }) => {
            json_no_store(
                StatusCode::UNAUTHORIZED,
                InvalidCredentialsResponse {
                    error: "invalid_credentials",
                    attempts_remaining,
                },
            )
        }
        Err(password_login::PasswordLoginError::Locked {
            retry_after_seconds,
        }) => {
            let mut response = json_no_store(
                StatusCode::TOO_MANY_REQUESTS,
                LockedResponse {
                    error: "locked",
                    retry_after_seconds,
                },
            );
            if let Ok(value) = HeaderValue::from_str(&retry_after_seconds.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
            response
        }
        Err(password_login::PasswordLoginError::Busy) => busy(),
        Err(password_login::PasswordLoginError::Persistence) => unavailable(),
    }
}

pub fn routes() -> Routes {
    Routes::new()
        .get("/auth/bootstrap", bootstrap)
        .post("/auth/register", register)
        .post("/auth/password/login", password_login)
}
