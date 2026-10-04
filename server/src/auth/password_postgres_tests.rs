//! Email/password registration and sign-in against real PostgreSQL.

use std::{error::Error, path::Path, time::Instant};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    middleware,
    routing::{delete, get, post},
};
use loco_rs::{
    app::{AppContext, Hooks},
    config::Config,
    environment::Environment,
};
use sea_orm::EntityTrait;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::{
    actions::{password_login, reset_password, validate_access},
    credentials::SetupCode,
    entities::password_credential,
    request::require_access_session,
    throttle::MAX_FAILURES,
};
use crate::{app::App, test_support::PostgresTestDatabase};

const EMAIL: &str = "Owner@Example.com";
const PASSWORD: &str = "correct horse battery staple";
const SETUP_CODE: &str = "0123456789abcdef0123456789abcdef";

async fn test_context(database: sea_orm::DatabaseConnection) -> Result<AppContext, Box<dyn Error>> {
    let config = Config::from_folder(&Environment::Test, Path::new("config"))?;
    let context = AppContext::builder(Environment::Test, database, config).build();
    let context = App::after_context(context).await?;
    // Replace the random startup code with one the tests can send.
    context.shared_store.insert(SetupCode::known(SETUP_CODE));
    Ok(context)
}

fn router(context: AppContext) -> Router {
    let protected = Router::new()
        .route(
            "/api/v1/auth/session",
            get(crate::controllers::account::current),
        )
        .layer(middleware::from_fn_with_state(
            context.db.clone(),
            require_access_session,
        ));
    Router::new()
        .route(
            "/api/v1/auth/bootstrap",
            get(crate::controllers::password_auth::bootstrap),
        )
        .route(
            "/api/v1/auth/register",
            post(crate::controllers::password_auth::register),
        )
        .route(
            "/api/v1/auth/password/login",
            post(crate::controllers::password_auth::password_login),
        )
        .route(
            "/api/v1/auth/tokens/refresh",
            post(crate::controllers::sessions::refresh),
        )
        .route(
            "/api/v1/auth/sessions/current",
            delete(crate::controllers::sessions::logout_current),
        )
        .route(
            "/api/v1/auth/github/login-attempts",
            post(crate::controllers::github_login::start),
        )
        .merge(protected)
        .with_state(context)
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    body: Option<Value>,
    bearer: Option<&str>,
) -> (StatusCode, Value, header::HeaderMap) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(bearer) = bearer {
        request = request.header(header::AUTHORIZATION, format!("Bearer {bearer}"));
    }
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    // Framework rejections (such as unknown JSON fields) answer in plain text.
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()))
    };
    (status, value, headers)
}

fn registration() -> Value {
    json!({ "displayName": "Ada Owner", "email": EMAIL, "password": PASSWORD, "setupCode": SETUP_CODE })
}

fn login(email: &str, password: &str) -> Value {
    json!({ "email": email, "password": password })
}

async fn with_app<F, Fut>(test: F) -> Result<(), Box<dyn Error>>
where
    F: FnOnce(Router, sea_orm::DatabaseConnection) -> Fut,
    Fut: std::future::Future<Output = Result<(), Box<dyn Error>>>,
{
    let Some(database) = PostgresTestDatabase::create(true).await? else {
        return Ok(());
    };
    let context = test_context(database.connection.clone()).await?;
    let result = test(router(context), database.connection.clone()).await;
    assert!(database.cleanup().await?);
    result
}

#[tokio::test]
async fn first_registration_creates_the_owner_and_the_second_is_refused()
-> Result<(), Box<dyn Error>> {
    with_app(|app, _| async move {
        let (status, body, _) = call(&app, "GET", "/api/v1/auth/bootstrap", None, None).await;
        assert_eq!(
            (status, body),
            (StatusCode::OK, json!({ "ownerExists": false }))
        );

        let (status, body, headers) = call(
            &app,
            "POST",
            "/api/v1/auth/register",
            Some(registration()),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        assert_eq!(body["email"], EMAIL);
        assert_eq!(body["displayName"], "Ada Owner");
        assert_eq!(body["clusterId"], "test");
        for field in [
            "principalId",
            "accessCredential",
            "refreshCredential",
            "accessExpiresAt",
            "refreshExpiresAt",
        ] {
            assert!(body[field].is_string(), "{field}");
        }
        assert!(body.get("password").is_none());

        let (status, body, _) = call(&app, "GET", "/api/v1/auth/bootstrap", None, None).await;
        assert_eq!(
            (status, body),
            (StatusCode::OK, json!({ "ownerExists": true }))
        );

        let (status, body, _) = call(
            &app,
            "POST",
            "/api/v1/auth/register",
            Some(
                json!({ "displayName": "Mallory", "email": "m@example.com", "password": PASSWORD, "setupCode": SETUP_CODE }),
            ),
            None,
        )
        .await;
        assert_eq!(
            (status, body),
            (StatusCode::CONFLICT, json!({ "error": "owner_exists" }))
        );
        Ok(())
    })
    .await
}

#[tokio::test]
async fn registration_needs_the_setup_code_and_a_wrong_one_never_creates_the_owner()
-> Result<(), Box<dyn Error>> {
    with_app(|app, _| async move {
        let mut wrong = registration();
        wrong["setupCode"] = json!("ffffffffffffffffffffffffffffffff");
        let (status, body, _) =
            call(&app, "POST", "/api/v1/auth/register", Some(wrong), None).await;
        assert_eq!(
            (status, body),
            (
                StatusCode::BAD_REQUEST,
                json!({ "error": "validation_failed", "fields": { "setupCode": "invalid" } })
            )
        );
        let (_, body, _) = call(&app, "GET", "/api/v1/auth/bootstrap", None, None).await;
        assert_eq!(body, json!({ "ownerExists": false }));

        // Pasted with surrounding spaces or in capitals still matches.
        let mut pasted = registration();
        pasted["setupCode"] = json!(format!("  {}  ", SETUP_CODE.to_uppercase()));
        let (status, _, _) = call(&app, "POST", "/api/v1/auth/register", Some(pasted), None).await;
        assert_eq!(status, StatusCode::CREATED);
        Ok(())
    })
    .await
}

#[tokio::test]
async fn concurrent_wrong_logins_get_at_most_ten_verifications() -> Result<(), Box<dyn Error>> {
    with_app(|app, _| async move {
        call(
            &app,
            "POST",
            "/api/v1/auth/register",
            Some(registration()),
            None,
        )
        .await;
        let attempts = (0..20).map(|_| {
            let app = app.clone();
            async move {
                call(
                    &app,
                    "POST",
                    "/api/v1/auth/password/login",
                    Some(login(EMAIL, "wrong horse battery staple")),
                    None,
                )
                .await
                .0
            }
        });
        let statuses = futures_util::future::join_all(attempts).await;
        let verified = statuses
            .iter()
            .filter(|status| **status == StatusCode::UNAUTHORIZED)
            .count();
        let refused = statuses
            .iter()
            .filter(|status| **status == StatusCode::TOO_MANY_REQUESTS)
            .count();
        assert!(verified <= MAX_FAILURES as usize, "{statuses:?}");
        assert_eq!(verified + refused, 20, "{statuses:?}");
        Ok(())
    })
    .await
}

#[tokio::test]
async fn a_sign_in_verified_against_a_reset_password_gets_no_session() -> Result<(), Box<dyn Error>>
{
    with_app(|app, database| async move {
        let (_, registered, _) = call(
            &app,
            "POST",
            "/api/v1/auth/register",
            Some(registration()),
            None,
        )
        .await;
        let principal_id: uuid::Uuid = registered["principalId"].as_str().unwrap().parse()?;
        let verified_at = password_credential::Entity::find_by_id(principal_id)
            .one(&database)
            .await?
            .unwrap()
            .updated_at;

        // The reset lands while the old password is being verified.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        reset_password::execute(&database, EMAIL, "a brand new passphrase".to_owned())
            .await
            .unwrap();
        let issued =
            password_login::create_if_credential_unchanged(&database, principal_id, verified_at)
                .await
                .unwrap();
        assert!(issued.is_none());

        let current = password_credential::Entity::find_by_id(principal_id)
            .one(&database)
            .await?
            .unwrap()
            .updated_at;
        let issued =
            password_login::create_if_credential_unchanged(&database, principal_id, current)
                .await
                .unwrap();
        assert!(issued.is_some());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn registration_and_login_report_field_errors_without_echoing_input()
-> Result<(), Box<dyn Error>> {
    with_app(|app, _| async move {
        let (status, body, _) = call(&app, "POST", "/api/v1/auth/register", Some(json!({ "displayName": " ", "email": "not-an-email", "password": "short-pass", "setupCode": " " })), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body, json!({ "error": "validation_failed", "fields": { "email": "invalid", "password": "too_short", "displayName": "empty", "setupCode": "empty" } }));
        assert!(!body.to_string().contains("short-pass"));

        let long = "x".repeat(129);
        let (status, body, _) = call(&app, "POST", "/api/v1/auth/password/login", Some(login("owner@example.com", &long)), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["fields"], json!({ "password": "too_long" }));

        let (status, _, _) = call(&app, "POST", "/api/v1/auth/register", Some(json!({ "displayName": "A", "email": "a@b.co", "password": PASSWORD, "role": "admin" })), None).await;
        assert!(status.is_client_error(), "unknown fields are rejected");
        Ok(())
    })
    .await
}

#[tokio::test]
async fn login_succeeds_case_insensitively_and_failures_count_down() -> Result<(), Box<dyn Error>> {
    with_app(|app, _| async move {
        call(
            &app,
            "POST",
            "/api/v1/auth/register",
            Some(registration()),
            None,
        )
        .await;

        let (status, body, _) = call(
            &app,
            "POST",
            "/api/v1/auth/password/login",
            Some(login("owner@EXAMPLE.com", PASSWORD)),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["displayName"], "Ada Owner");

        let (status, body, _) = call(
            &app,
            "POST",
            "/api/v1/auth/password/login",
            Some(login(EMAIL, "wrong horse battery staple")),
            None,
        )
        .await;
        assert_eq!(
            (status, body),
            (
                StatusCode::UNAUTHORIZED,
                json!({ "error": "invalid_credentials", "attemptsRemaining": 9 })
            )
        );

        // An unknown email gets the identical body shape from its own counter.
        let (status, body, _) = call(
            &app,
            "POST",
            "/api/v1/auth/password/login",
            Some(login("nobody@example.com", PASSWORD)),
            None,
        )
        .await;
        assert_eq!(
            (status, body),
            (
                StatusCode::UNAUTHORIZED,
                json!({ "error": "invalid_credentials", "attemptsRemaining": 9 })
            )
        );

        // Success clears the counter.
        call(
            &app,
            "POST",
            "/api/v1/auth/password/login",
            Some(login(EMAIL, PASSWORD)),
            None,
        )
        .await;
        let (_, body, _) = call(
            &app,
            "POST",
            "/api/v1/auth/password/login",
            Some(login(EMAIL, "wrong horse battery staple")),
            None,
        )
        .await;
        assert_eq!(body["attemptsRemaining"], 9);
        Ok(())
    })
    .await
}

#[tokio::test]
async fn a_non_ascii_owner_email_signs_in_in_any_case() -> Result<(), Box<dyn Error>> {
    with_app(|app, database| async move {
        // Rust lowers `İ` to `i` plus a combining dot; PostgreSQL lowers it to a plain `i`.
        // Lowering only one side in Rust locked this owner out even with the exact email.
        let email = "İnci.Ünal@Exämple.com";
        let mut registration = registration();
        registration["email"] = json!(email);
        let (status, _, _) = call(
            &app,
            "POST",
            "/api/v1/auth/register",
            Some(registration),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        // The compose database's en_US.utf8 collation lowers `Ü` and `Ä` too.
        for typed in [email, " İNCI.ünal@exÄmple.COM "] {
            let (status, body, _) = call(
                &app,
                "POST",
                "/api/v1/auth/password/login",
                Some(login(typed, PASSWORD)),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{typed}: {body}");
        }
        assert!(
            reset_password::execute(
                &database,
                "İNCI.ÜNAL@EXÄMPLE.COM",
                "a brand new passphrase".to_owned()
            )
            .await
            .is_ok()
        );
        Ok(())
    })
    .await
}

#[tokio::test]
async fn an_unknown_email_costs_the_same_hashing_work_as_a_wrong_password()
-> Result<(), Box<dyn Error>> {
    with_app(|app, _| async move {
        call(
            &app,
            "POST",
            "/api/v1/auth/register",
            Some(registration()),
            None,
        )
        .await;
        // Warm the equalizer hash so its one-time construction is not measured.
        call(
            &app,
            "POST",
            "/api/v1/auth/password/login",
            Some(login("warmup@example.com", PASSWORD)),
            None,
        )
        .await;

        let started = Instant::now();
        call(
            &app,
            "POST",
            "/api/v1/auth/password/login",
            Some(login(EMAIL, "wrong horse battery staple")),
            None,
        )
        .await;
        let wrong_password = started.elapsed();
        let started = Instant::now();
        call(
            &app,
            "POST",
            "/api/v1/auth/password/login",
            Some(login("stranger@example.com", PASSWORD)),
            None,
        )
        .await;
        let unknown_email = started.elapsed();

        // Same timing class: both pay one Argon2id verification. The bound is
        // loose so scheduler noise cannot flake it, but a skipped hash (≈1 ms
        // against ≈20+ ms) fails it.
        assert!(
            unknown_email * 3 >= wrong_password,
            "unknown {unknown_email:?} vs wrong {wrong_password:?}"
        );
        assert!(
            wrong_password * 3 >= unknown_email,
            "unknown {unknown_email:?} vs wrong {wrong_password:?}"
        );
        Ok(())
    })
    .await
}

#[tokio::test]
async fn ten_failures_lock_the_email_and_a_password_reset_unlocks_it() -> Result<(), Box<dyn Error>>
{
    with_app(|app, database| async move {
        call(
            &app,
            "POST",
            "/api/v1/auth/register",
            Some(registration()),
            None,
        )
        .await;
        for attempt in 1..MAX_FAILURES {
            let (status, body, _) = call(
                &app,
                "POST",
                "/api/v1/auth/password/login",
                Some(login(EMAIL, "wrong horse battery staple")),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
            assert_eq!(body["attemptsRemaining"], MAX_FAILURES - attempt);
        }
        let (status, body, headers) = call(
            &app,
            "POST",
            "/api/v1/auth/password/login",
            Some(login(EMAIL, "wrong horse battery staple")),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["error"], "locked");
        let retry_after = body["retryAfterSeconds"].as_u64().unwrap();
        assert!((1..=900).contains(&retry_after));
        assert_eq!(headers[header::RETRY_AFTER], retry_after.to_string());

        // Even the right password is refused while locked.
        let (status, _, _) = call(
            &app,
            "POST",
            "/api/v1/auth/password/login",
            Some(login(EMAIL, PASSWORD)),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);

        // The CLI reset runs in another process; the newer credential voids the lock.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        reset_password::execute(&database, EMAIL, "a brand new passphrase".to_owned())
            .await
            .unwrap();
        let (status, _, _) = call(
            &app,
            "POST",
            "/api/v1/auth/password/login",
            Some(login(EMAIL, "a brand new passphrase")),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        Ok(())
    })
    .await
}

#[tokio::test]
async fn password_sessions_refresh_revoke_and_identify_like_every_session()
-> Result<(), Box<dyn Error>> {
    with_app(|app, database| async move {
        let (_, registered, _) = call(&app, "POST", "/api/v1/auth/register", Some(registration()), None).await;
        let access = registered["accessCredential"].as_str().unwrap().to_owned();

        let (status, account, _) = call(&app, "GET", "/api/v1/auth/session", None, Some(&access)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(account, json!({ "principalId": registered["principalId"], "email": EMAIL, "displayName": "Ada Owner" }));
        let (status, _, _) = call(&app, "GET", "/api/v1/auth/session", None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, rotated, _) = call(&app, "POST", "/api/v1/auth/tokens/refresh", Some(json!({ "refreshCredential": registered["refreshCredential"] })), None).await;
        assert_eq!(status, StatusCode::OK);
        let rotated_access = rotated["accessCredential"].as_str().unwrap().to_owned();
        assert!(validate_access::execute(&database, &rotated_access).await.is_ok());

        let (status, _, _) = call(&app, "DELETE", "/api/v1/auth/sessions/current", None, Some(&rotated_access)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(validate_access::execute(&database, &rotated_access).await.is_err());
        Ok(())
    })
    .await
}

#[tokio::test]
async fn reset_rehashes_and_revokes_every_session() -> Result<(), Box<dyn Error>> {
    with_app(|app, database| async move {
        let (_, registered, _) = call(
            &app,
            "POST",
            "/api/v1/auth/register",
            Some(registration()),
            None,
        )
        .await;
        let (_, second, _) = call(
            &app,
            "POST",
            "/api/v1/auth/password/login",
            Some(login(EMAIL, PASSWORD)),
            None,
        )
        .await;

        assert_eq!(
            reset_password::execute(&database, EMAIL, "short".to_owned()).await,
            Err(reset_password::ResetPasswordError::InvalidPassword(
                super::validation::FieldProblem::TooShort
            ))
        );
        assert_eq!(
            reset_password::execute(
                &database,
                "nobody@example.com",
                "a brand new passphrase".to_owned()
            )
            .await,
            Err(reset_password::ResetPasswordError::UnknownEmail)
        );
        assert_eq!(
            reset_password::execute(
                &database,
                "owner@example.COM",
                "a brand new passphrase".to_owned()
            )
            .await,
            Ok(2)
        );

        for session in [&registered, &second] {
            assert!(
                validate_access::execute(&database, session["accessCredential"].as_str().unwrap())
                    .await
                    .is_err()
            );
            let (status, _, _) = call(
                &app,
                "POST",
                "/api/v1/auth/tokens/refresh",
                Some(json!({ "refreshCredential": session["refreshCredential"] })),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
        }
        let (status, _, _) = call(
            &app,
            "POST",
            "/api/v1/auth/password/login",
            Some(login(EMAIL, PASSWORD)),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _, _) = call(
            &app,
            "POST",
            "/api/v1/auth/password/login",
            Some(login(EMAIL, "a brand new passphrase")),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        Ok(())
    })
    .await
}

#[tokio::test]
async fn github_login_stays_compiled_and_answers_503_while_disabled() -> Result<(), Box<dyn Error>>
{
    with_app(|app, _| async move {
        let (status, _, _) = call(
            &app,
            "POST",
            "/api/v1/auth/github/login-attempts",
            Some(json!({ "codeChallenge": "x".repeat(43) })),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        Ok(())
    })
    .await
}
