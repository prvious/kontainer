//! Password sign-in, owner registration, bootstrap, account, and probe contracts.

use std::collections::BTreeSet;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::support::{
    Behavior, TestServer, assert_openapi_snapshot, assert_schema_fields, client_for,
    covered_contract, openapi_artifact, plain_client, respond, response_statuses, round_trip_keys,
    serialized_keys, test_pki, tls_client, wait,
};
use crate::auth::{
    AccountReply, BootstrapReply, PasswordLoginBody, PasswordSessionReply, RegisterOwnerBody,
};
use crate::client::TrustPolicy;
use crate::credentials::AccessCredential;
use crate::error::{AccountFieldErrors, FieldProblem, NativeClientError};
use crate::probe::ProbeFailure;

const LOGIN_REQUEST: &str = include_str!("fixtures/auth/password-login-request.json");
const REGISTER_REQUEST: &str = include_str!("fixtures/auth/register-owner-request.json");
const SESSION_RESPONSE: &str = include_str!("fixtures/auth/password-session-response.json");
const BOOTSTRAP_RESPONSE: &str = include_str!("fixtures/auth/bootstrap-response.json");
const ACCOUNT_RESPONSE: &str = include_str!("fixtures/auth/account-response.json");
const EMAIL: &str = "owner@example.com";
const PASSWORD: &str = "correct horse battery staple";
const ACCESS_CANARY: &str = "kta_11111111111111111111111111111111.2222222222222222222222222222222222222222222222222222222222222222";

fn with_request_id(status: u16, body: &str, request_id: &str) -> Behavior {
    Behavior::RespondWithHeaders {
        status,
        body: body.to_owned(),
        headers: vec![("x-request-id", request_id.to_owned())],
    }
}

fn json_body(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("JSON")
}

#[tokio::test]
async fn password_login_sends_the_body_once_and_reads_the_session() {
    let server = TestServer::plain(vec![respond(200, SESSION_RESPONSE)]).await;
    let client = plain_client(&server);

    let session = wait(client.password_login(EMAIL, PASSWORD, &CancellationToken::new()))
        .await
        .expect("signed in");
    assert_eq!(session.cluster_id, "cluster-1");
    assert_eq!(session.email, EMAIL);
    assert_eq!(session.display_name, "Ada Owner");
    assert_eq!(session.credentials.access.expose(), ACCESS_CANARY);

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].target, "/api/v1/auth/password/login");
    assert!(requests[0].header("authorization").is_none());
    assert!(!requests[0].target.contains(PASSWORD));
    assert_eq!(json_body(&requests[0].body), json_body(LOGIN_REQUEST));
}

#[tokio::test]
async fn sign_in_failures_carry_their_closed_details_and_the_request_id() {
    let server = TestServer::plain(vec![
        with_request_id(
            401,
            r#"{"error":"invalid_credentials","attemptsRemaining":7}"#,
            "req-0001",
        ),
        with_request_id(
            429,
            r#"{"error":"locked","retryAfterSeconds":600}"#,
            "req-0002",
        ),
        respond(
            400,
            r#"{"error":"validation_failed","fields":{"email":"invalid","password":"too_short"}}"#,
        ),
        with_request_id(401, r#"{"error":"invalid_credentials"}"#, "has spaces"),
    ])
    .await;
    let client = plain_client(&server);
    let cancel = CancellationToken::new();

    let error = wait(client.password_login(EMAIL, "wrong", &cancel))
        .await
        .unwrap_err();
    assert_eq!(
        error.error,
        NativeClientError::InvalidCredentials {
            attempts_remaining: 7
        }
    );
    assert_eq!(error.request_id.as_deref(), Some("req-0001"));

    let error = wait(client.password_login(EMAIL, "wrong", &cancel))
        .await
        .unwrap_err();
    assert_eq!(
        error.error,
        NativeClientError::LoginLocked {
            retry_after_seconds: 600
        }
    );

    let error = wait(client.password_login("x", "y", &cancel))
        .await
        .unwrap_err();
    assert_eq!(
        error.error,
        NativeClientError::ValidationFailed {
            fields: AccountFieldErrors {
                email: Some(FieldProblem::Invalid),
                password: Some(FieldProblem::TooShort),
                display_name: None
            }
        }
    );
    assert_eq!(error.request_id, None);

    // An envelope missing its detail falls back to the plain status code, and
    // an unsafe request id is dropped rather than passed along.
    let error = wait(client.password_login(EMAIL, "wrong", &cancel))
        .await
        .unwrap_err();
    assert_eq!(error.error, NativeClientError::Unauthorized);
    assert_eq!(error.request_id, None);
    assert_eq!(server.request_count(), 4, "sign-in is never retried");
}

#[tokio::test]
async fn register_owner_creates_the_session_or_reports_that_registration_closed() {
    let server = TestServer::plain(vec![
        respond(201, SESSION_RESPONSE),
        respond(409, r#"{"error":"owner_exists"}"#),
    ])
    .await;
    let client = plain_client(&server);
    let cancel = CancellationToken::new();

    let session = wait(client.register_owner("Ada Owner", EMAIL, PASSWORD, &cancel))
        .await
        .expect("registered");
    assert_eq!(session.display_name, "Ada Owner");
    let requests = server.requests();
    assert_eq!(requests[0].target, "/api/v1/auth/register");
    assert_eq!(json_body(&requests[0].body), json_body(REGISTER_REQUEST));

    let error = wait(client.register_owner("Ada Owner", EMAIL, PASSWORD, &cancel))
        .await
        .unwrap_err();
    assert_eq!(error.error, NativeClientError::OwnerExists);
}

#[tokio::test]
async fn bootstrap_status_is_anonymous_and_not_retried() {
    let server = TestServer::plain(vec![respond(200, BOOTSTRAP_RESPONSE), Behavior::Reset]).await;
    let client = plain_client(&server);
    assert!(
        wait(client.bootstrap_status(&CancellationToken::new()))
            .await
            .unwrap()
    );
    let error = wait(client.bootstrap_status(&CancellationToken::new()))
        .await
        .unwrap_err();
    assert_eq!(error.error, NativeClientError::Unavailable);
    assert_eq!(server.request_count(), 2);
    assert!(server.requests()[0].header("authorization").is_none());
}

#[tokio::test]
async fn current_session_presents_the_bearer_and_reads_the_account() {
    let server = TestServer::plain(vec![respond(200, ACCOUNT_RESPONSE)]).await;
    let client = plain_client(&server);
    let access = AccessCredential::parse(ACCESS_CANARY).unwrap();
    let account = wait(client.current_session(&access, &CancellationToken::new()))
        .await
        .unwrap();
    assert_eq!(account.email.as_deref(), Some(EMAIL));
    assert_eq!(account.display_name.as_deref(), Some("Ada Owner"));
    let requests = server.requests();
    assert_eq!(requests[0].target, "/api/v1/auth/session");
    assert_eq!(
        requests[0].header("authorization"),
        Some(format!("Bearer {ACCESS_CANARY}").as_str())
    );
}

#[tokio::test]
async fn probe_names_the_step_that_failed() {
    let ready = TestServer::plain(vec![
        respond(200, r#"{"ok":true}"#),
        respond(503, r#"{"ok":false}"#),
    ])
    .await;
    let client = plain_client(&ready);
    assert_eq!(
        wait(client.probe_ready(&CancellationToken::new())).await,
        Ok(())
    );
    assert_eq!(
        wait(client.probe_ready(&CancellationToken::new())).await,
        Err(ProbeFailure::Http)
    );
    assert_eq!(ready.requests()[0].target, "/health/ready");

    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let client = client_for(
        &format!("http://127.0.0.1:{port}"),
        TrustPolicy::WebPki,
        None,
    );
    assert_eq!(
        wait(client.probe_ready(&CancellationToken::new())).await,
        Err(ProbeFailure::Tcp)
    );

    let pki = test_pki();
    let untrusted = TestServer::tls(
        pki.server_config.clone(),
        vec![respond(200, r#"{"ok":true}"#)],
    )
    .await;
    let client = tls_client(&untrusted, &pki.other_ca_pem);
    assert_eq!(
        wait(client.probe_ready(&CancellationToken::new())).await,
        Err(ProbeFailure::Tls)
    );

    let stall = TestServer::plain(vec![Behavior::Stall]).await;
    let client = client_for(
        &stall.origin(),
        TrustPolicy::WebPki,
        Some(super::support::SHORT_DEADLINE),
    );
    assert_eq!(
        wait(client.probe_ready(&CancellationToken::new())).await,
        Err(ProbeFailure::Timeout)
    );

    let client = client_for(
        "https://kontainer-probe.invalid",
        TrustPolicy::WebPki,
        Some(std::time::Duration::from_secs(2)),
    );
    let outcome = wait(client.probe_ready(&CancellationToken::new())).await;
    assert!(
        matches!(outcome, Err(ProbeFailure::Dns | ProbeFailure::Timeout)),
        "{outcome:?}"
    );
}

const PASSWORD_OPENAPI_PATHS: [(&str, &str); 4] = [
    ("/api/v1/auth/bootstrap", "get"),
    ("/api/v1/auth/register", "post"),
    ("/api/v1/auth/password/login", "post"),
    ("/api/v1/auth/session", "get"),
];
const PASSWORD_OPENAPI_SCHEMAS: [&str; 8] = [
    "AccountResponse",
    "BootstrapResponse",
    "InvalidCredentialsResponse",
    "LockedResponse",
    "PasswordLoginRequest",
    "PasswordSessionResponse",
    "RegisterRequest",
    "ValidationErrorResponse",
];

#[test]
fn openapi_password_contract_matches_the_checked_artifact() {
    let document = openapi_artifact();
    assert_openapi_snapshot(
        "auth/password-openapi-contract.json",
        &covered_contract(
            &document,
            &PASSWORD_OPENAPI_PATHS,
            &PASSWORD_OPENAPI_SCHEMAS,
        ),
    );

    for (path, method) in &PASSWORD_OPENAPI_PATHS[..3] {
        assert_eq!(
            document["paths"][path][method]["security"],
            json!([{}]),
            "{method} {path} must be anonymous"
        );
    }
    assert!(
        document["paths"]["/api/v1/auth/session"]["get"]
            .get("security")
            .is_none(),
        "the account read needs a session"
    );
    assert_eq!(
        response_statuses(&document, "/api/v1/auth/password/login", "post"),
        BTreeSet::from(["200", "400", "401", "429", "503"].map(str::to_owned))
    );
    assert_eq!(
        response_statuses(&document, "/api/v1/auth/register", "post"),
        BTreeSet::from(["201", "400", "409", "503"].map(str::to_owned))
    );

    assert_schema_fields(
        &document,
        "PasswordLoginRequest",
        serialized_keys(&PasswordLoginBody {
            email: "fixture",
            password: "fixture",
        }),
    );
    assert_schema_fields(
        &document,
        "RegisterRequest",
        serialized_keys(&RegisterOwnerBody {
            display_name: "fixture",
            email: "fixture",
            password: "fixture",
        }),
    );
    assert_schema_fields(
        &document,
        "PasswordSessionResponse",
        round_trip_keys::<PasswordSessionReply>(SESSION_RESPONSE),
    );
    assert_schema_fields(
        &document,
        "BootstrapResponse",
        round_trip_keys::<BootstrapReply>(BOOTSTRAP_RESPONSE),
    );
    assert_schema_fields(
        &document,
        "AccountResponse",
        round_trip_keys::<AccountReply>(ACCOUNT_RESPONSE),
    );
}
