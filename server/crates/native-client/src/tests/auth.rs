//! Login attempt, redemption, refresh, and logout contracts.

use std::collections::BTreeSet;

use chrono::SecondsFormat;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::support::{
    Behavior, TestServer, assert_openapi_snapshot, assert_schema_fields, covered_contract,
    openapi_artifact, plain_client, respond, response_statuses, round_trip_keys, serialized_keys,
    wait,
};
use crate::auth::{
    RedeemLoginBody, RedeemLoginReply, RefreshSessionBody, RefreshSessionReply, StartLoginBody,
    StartLoginReply,
};
use crate::credentials::{AccessCredential, LoginVerifier, RefreshCredential};
use crate::error::NativeClientError;

const START_LOGIN_REQUEST: &str = include_str!("fixtures/auth/start-login-request.json");
const START_LOGIN_RESPONSE: &str = include_str!("fixtures/auth/start-login-response.json");
const REDEEM_LOGIN_REQUEST: &str = include_str!("fixtures/auth/redeem-login-request.json");
const REDEEM_LOGIN_RESPONSE: &str = include_str!("fixtures/auth/redeem-login-response.json");
const REFRESH_REQUEST: &str = include_str!("fixtures/auth/refresh-request.json");
const REFRESH_RESPONSE: &str = include_str!("fixtures/auth/refresh-response.json");

/// RFC 7636 Appendix B verifier; its challenge is in the start fixture.
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const ATTEMPT_ID: &str = "018f2e34-0000-7000-8000-000000000001";
const PRINCIPAL_ID: &str = "018f2e34-0000-7000-8000-0000000000aa";
const ACCESS_CANARY: &str = "kta_11111111111111111111111111111111.2222222222222222222222222222222222222222222222222222222222222222";
const REFRESH_CANARY: &str = "ktr_33333333333333333333333333333333.4444444444444444444444444444444444444444444444444444444444444444";
const ROTATED_ACCESS_CANARY: &str = "kta_77777777777777777777777777777777.8888888888888888888888888888888888888888888888888888888888888888";
const ROTATED_REFRESH_CANARY: &str = "ktr_55555555555555555555555555555555.6666666666666666666666666666666666666666666666666666666666666666";

fn assert_json_eq(actual: &str, expected: &str) {
    let actual: serde_json::Value = serde_json::from_str(actual).expect("captured body is JSON");
    let expected: serde_json::Value = serde_json::from_str(expected).expect("fixture is JSON");
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn start_login_sends_the_rfc7636_challenge_and_reads_the_attempt() {
    let server = TestServer::plain(vec![respond(201, START_LOGIN_RESPONSE)]).await;
    let client = plain_client(&server);
    let verifier = LoginVerifier::parse(VERIFIER).unwrap();

    let attempt = wait(client.start_login(&verifier, &CancellationToken::new()))
        .await
        .expect("login attempt starts");

    assert_eq!(attempt.attempt_id, Uuid::parse_str(ATTEMPT_ID).unwrap());
    assert!(
        attempt
            .authorize_url
            .as_str()
            .starts_with("https://github.com/login/oauth/authorize?")
    );
    assert_eq!(
        attempt
            .expires_at
            .to_rfc3339_opts(SecondsFormat::Millis, true),
        "2026-09-19T06:57:00.123Z"
    );

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].target, "/api/v1/auth/github/login-attempts");
    assert_eq!(requests[0].header("content-type"), Some("application/json"));
    assert!(
        requests[0].header("authorization").is_none(),
        "login start is anonymous"
    );
    assert_json_eq(&requests[0].body, START_LOGIN_REQUEST);
    assert!(!requests[0].target.contains(VERIFIER));
    assert!(
        !requests[0].body.contains(VERIFIER),
        "the verifier itself never crosses the start endpoint"
    );
}

#[tokio::test]
async fn redeem_login_sends_the_verifier_body_and_reads_typed_credentials() {
    let server = TestServer::plain(vec![respond(200, REDEEM_LOGIN_RESPONSE)]).await;
    let client = plain_client(&server);
    let verifier = LoginVerifier::parse(VERIFIER).unwrap();
    let attempt_id = Uuid::parse_str(ATTEMPT_ID).unwrap();

    let session = wait(client.redeem_login(attempt_id, &verifier, &CancellationToken::new()))
        .await
        .expect("redemption succeeds");

    assert_eq!(session.cluster_id, "cluster-1");
    assert_eq!(session.principal_id, Uuid::parse_str(PRINCIPAL_ID).unwrap());
    assert_eq!(session.github_login, "clvsh");
    assert_eq!(session.credentials.access.expose(), ACCESS_CANARY);
    assert_eq!(session.credentials.refresh.expose(), REFRESH_CANARY);
    assert_eq!(
        session
            .credentials
            .access_expires_at
            .to_rfc3339_opts(SecondsFormat::Millis, true),
        "2026-09-19T07:02:00.123Z"
    );
    assert_eq!(
        session
            .credentials
            .refresh_expires_at
            .to_rfc3339_opts(SecondsFormat::Millis, true),
        "2026-10-19T06:57:00.123Z"
    );

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(
        requests[0].target,
        format!("/api/v1/auth/github/login-attempts/{ATTEMPT_ID}/redeem")
    );
    assert!(requests[0].header("authorization").is_none());
    assert_json_eq(&requests[0].body, REDEEM_LOGIN_REQUEST);
    assert!(!requests[0].target.contains(VERIFIER));
}

#[tokio::test]
async fn refresh_session_rotates_the_credential_pair() {
    let server = TestServer::plain(vec![respond(200, REFRESH_RESPONSE)]).await;
    let client = plain_client(&server);
    let refresh = RefreshCredential::parse(REFRESH_CANARY).unwrap();

    let credentials = wait(client.refresh_session(&refresh, &CancellationToken::new()))
        .await
        .expect("rotation succeeds");

    assert_eq!(credentials.access.expose(), ROTATED_ACCESS_CANARY);
    assert_eq!(credentials.refresh.expose(), ROTATED_REFRESH_CANARY);

    let requests = server.requests();
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].target, "/api/v1/auth/tokens/refresh");
    assert!(requests[0].header("authorization").is_none());
    assert_json_eq(&requests[0].body, REFRESH_REQUEST);
    assert!(
        !requests[0].target.contains(REFRESH_CANARY),
        "refresh credentials travel in the body only"
    );
}

#[tokio::test]
async fn logout_presents_a_bearer_header_and_accepts_no_content() {
    let server = TestServer::plain(vec![respond(204, "")]).await;
    let client = plain_client(&server);
    let access = AccessCredential::parse(ACCESS_CANARY).unwrap();

    wait(client.logout(&access, &CancellationToken::new()))
        .await
        .expect("logout succeeds");

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "DELETE");
    assert_eq!(requests[0].target, "/api/v1/auth/sessions/current");
    let expected = format!("Bearer {ACCESS_CANARY}");
    assert_eq!(requests[0].header("authorization"), Some(expected.as_str()));
    assert_eq!(requests[0].body, "");
}

#[tokio::test]
async fn logout_rejects_a_success_body() {
    let server = TestServer::plain(vec![respond(200, "{}")]).await;
    let client = plain_client(&server);
    let access = AccessCredential::parse(ACCESS_CANARY).unwrap();

    assert_eq!(
        client
            .logout(&access, &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::InvalidResponse)
    );
}

#[tokio::test]
async fn start_login_rejects_a_non_https_authorize_url() {
    let verifier = LoginVerifier::parse(VERIFIER).unwrap();
    for scheme in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "data:text/html,<script>alert(1)</script>",
        "kontainer://callback",
        "http://github.com/login/oauth/authorize",
    ] {
        let fixture = START_LOGIN_RESPONSE.replace(
            "https://github.com/login/oauth/authorize?",
            &format!("{scheme}?"),
        );
        let server = TestServer::plain(vec![respond(201, &fixture)]).await;
        let client = plain_client(&server);
        assert_eq!(
            client
                .start_login(&verifier, &CancellationToken::new())
                .await
                .err(),
            Some(NativeClientError::InvalidResponse),
            "{scheme} must never reach the system browser"
        );
    }
}

#[tokio::test]
async fn start_login_maps_documented_statuses() {
    let verifier = LoginVerifier::parse(VERIFIER).unwrap();
    for (status, expected) in [
        (400, NativeClientError::InvalidRequest),
        (503, NativeClientError::ServiceUnavailable),
        (500, NativeClientError::UnexpectedStatus { status: 500 }),
    ] {
        let server = TestServer::plain(vec![respond(status, r#"{"error":"fixture"}"#)]).await;
        let client = plain_client(&server);
        let error = client
            .start_login(&verifier, &CancellationToken::new())
            .await
            .expect_err("status maps to an error");
        assert_eq!(error, expected, "status {status}");
        assert_eq!(server.request_count(), 1, "status {status}");
    }
}

#[tokio::test]
async fn redeem_login_maps_documented_statuses() {
    let verifier = LoginVerifier::parse(VERIFIER).unwrap();
    let attempt_id = Uuid::parse_str(ATTEMPT_ID).unwrap();
    for (status, expected) in [
        (400, NativeClientError::InvalidRequest),
        (403, NativeClientError::Forbidden),
        (409, NativeClientError::Conflict),
        (502, NativeClientError::ProviderUnavailable),
        (503, NativeClientError::ServiceUnavailable),
    ] {
        let server = TestServer::plain(vec![respond(status, r#"{"error":"fixture"}"#)]).await;
        let client = plain_client(&server);
        let error = client
            .redeem_login(attempt_id, &verifier, &CancellationToken::new())
            .await
            .expect_err("status maps to an error");
        assert_eq!(error, expected, "status {status}");
        assert_eq!(server.request_count(), 1, "status {status}");
    }
}

#[tokio::test]
async fn refresh_and_logout_map_documented_statuses() {
    let refresh = RefreshCredential::parse(REFRESH_CANARY).unwrap();
    let access = AccessCredential::parse(ACCESS_CANARY).unwrap();
    for status in [401, 503] {
        let expected = if status == 401 {
            NativeClientError::Unauthorized
        } else {
            NativeClientError::ServiceUnavailable
        };
        let server = TestServer::plain(vec![respond(status, r#"{"error":"fixture"}"#)]).await;
        let client = plain_client(&server);
        assert_eq!(
            client
                .refresh_session(&refresh, &CancellationToken::new())
                .await
                .err(),
            Some(expected),
            "refresh status {status}"
        );
        assert_eq!(
            client
                .logout(&access, &CancellationToken::new())
                .await
                .err(),
            Some(expected),
            "logout status {status}"
        );
        assert_eq!(server.request_count(), 2, "status {status}");
    }
}

#[tokio::test]
async fn auth_calls_never_retry_transport_failures() {
    let verifier = LoginVerifier::parse(VERIFIER).unwrap();
    let attempt_id = Uuid::parse_str(ATTEMPT_ID).unwrap();
    let refresh = RefreshCredential::parse(REFRESH_CANARY).unwrap();
    let access = AccessCredential::parse(ACCESS_CANARY).unwrap();

    let server = TestServer::plain(vec![Behavior::Reset]).await;
    let client = plain_client(&server);
    assert_eq!(
        client
            .start_login(&verifier, &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::Unavailable)
    );
    assert_eq!(server.connection_count(), 1);

    let server = TestServer::plain(vec![Behavior::Reset]).await;
    let client = plain_client(&server);
    assert_eq!(
        client
            .redeem_login(attempt_id, &verifier, &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::Unavailable)
    );
    assert_eq!(server.connection_count(), 1);

    let server = TestServer::plain(vec![Behavior::Reset]).await;
    let client = plain_client(&server);
    assert_eq!(
        client
            .refresh_session(&refresh, &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::Unavailable)
    );
    assert_eq!(server.connection_count(), 1);

    let server = TestServer::plain(vec![Behavior::Reset]).await;
    let client = plain_client(&server);
    assert_eq!(
        client
            .logout(&access, &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::Unavailable)
    );
    assert_eq!(server.connection_count(), 1);
}

#[tokio::test]
async fn malformed_auth_success_bodies_are_rejected() {
    let verifier = LoginVerifier::parse(VERIFIER).unwrap();
    let attempt_id = Uuid::parse_str(ATTEMPT_ID).unwrap();

    let not_an_attempt = START_LOGIN_RESPONSE.replace(ATTEMPT_ID, "not-a-uuid");
    let server = TestServer::plain(vec![respond(201, &not_an_attempt)]).await;
    let client = plain_client(&server);
    assert_eq!(
        client
            .start_login(&verifier, &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::InvalidResponse)
    );

    let unknown_field =
        START_LOGIN_RESPONSE.replace("\"attemptId\"", "\"unexpected\":\"field\",\"attemptId\"");
    let server = TestServer::plain(vec![respond(201, &unknown_field)]).await;
    let client = plain_client(&server);
    assert_eq!(
        client
            .start_login(&verifier, &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::InvalidResponse),
        "unknown response fields are contract drift"
    );

    let bad_credential = REDEEM_LOGIN_RESPONSE.replace(ACCESS_CANARY, "not-a-kontainer-credential");
    let server = TestServer::plain(vec![respond(200, &bad_credential)]).await;
    let client = plain_client(&server);
    assert_eq!(
        client
            .redeem_login(attempt_id, &verifier, &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::InvalidResponse)
    );

    let bad_timestamp = REFRESH_RESPONSE.replace("2026-09-19T07:07:00.123Z", "not-a-timestamp");
    let server = TestServer::plain(vec![respond(200, &bad_timestamp)]).await;
    let client = plain_client(&server);
    let refresh = RefreshCredential::parse(REFRESH_CANARY).unwrap();
    assert_eq!(
        client
            .refresh_session(&refresh, &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::InvalidResponse)
    );
    assert_eq!(
        server.connection_count(),
        1,
        "a decode failure never retries"
    );
}

#[tokio::test]
async fn logout_does_not_forward_credentials_to_a_redirect_target() {
    let target = TestServer::plain(vec![respond(204, "")]).await;
    let location = format!("{}/auth/sessions/current", target.origin());
    let origin = TestServer::plain(vec![Behavior::RespondWithHeaders {
        status: 302,
        body: String::new(),
        headers: vec![("Location", location)],
    }])
    .await;
    let client = plain_client(&origin);
    let access = AccessCredential::parse(ACCESS_CANARY).unwrap();

    let error = client
        .logout(&access, &CancellationToken::new())
        .await
        .expect_err("a redirect is not a logout");
    assert_eq!(error, NativeClientError::UnexpectedStatus { status: 302 });
    assert_eq!(origin.request_count(), 1);
    assert_eq!(target.connection_count(), 0);
    assert!(!format!("{error:?}").contains("kta_"));
    assert!(!error.to_string().contains(ACCESS_CANARY));
}

/// Paths covered by the client's auth surface, with their documented method.
const AUTH_OPENAPI_PATHS: [(&str, &str); 4] = [
    ("/api/v1/auth/github/login-attempts", "post"),
    (
        "/api/v1/auth/github/login-attempts/{attempt_id}/redeem",
        "post",
    ),
    ("/api/v1/auth/tokens/refresh", "post"),
    ("/api/v1/auth/sessions/current", "delete"),
];
/// Login lifecycle endpoints carry their own credential checks and must never
/// inherit the global access-session scheme.
const ANONYMOUS_AUTH_OPENAPI_PATHS: [(&str, &str); 3] = [
    ("/api/v1/auth/github/login-attempts", "post"),
    (
        "/api/v1/auth/github/login-attempts/{attempt_id}/redeem",
        "post",
    ),
    ("/api/v1/auth/tokens/refresh", "post"),
];
const AUTH_OPENAPI_SCHEMAS: [&str; 6] = [
    "GithubLoginResponse",
    "RedeemLoginRequest",
    "RefreshRequest",
    "SessionTokenResponse",
    "StartLoginRequest",
    "StartLoginResponse",
];

#[test]
fn openapi_auth_contract_matches_the_checked_artifact() {
    let document = openapi_artifact();
    assert_openapi_snapshot(
        "auth/openapi-contract.json",
        &covered_contract(&document, &AUTH_OPENAPI_PATHS, &AUTH_OPENAPI_SCHEMAS),
    );

    for (path, method) in ANONYMOUS_AUTH_OPENAPI_PATHS {
        assert_eq!(
            document["paths"][path][method]["security"],
            json!([{}]),
            "{method} {path} must be explicitly anonymous"
        );
    }
    assert!(
        document["paths"]["/api/v1/auth/sessions/current"]["delete"]
            .get("security")
            .is_none(),
        "logout must inherit the global access-session scheme"
    );

    assert_eq!(
        response_statuses(&document, "/api/v1/auth/github/login-attempts", "post"),
        BTreeSet::from(["201", "400", "503"].map(str::to_owned))
    );
    assert_eq!(
        response_statuses(
            &document,
            "/api/v1/auth/github/login-attempts/{attempt_id}/redeem",
            "post"
        ),
        BTreeSet::from(["200", "400", "403", "409", "502", "503"].map(str::to_owned))
    );
    assert_eq!(
        response_statuses(&document, "/api/v1/auth/tokens/refresh", "post"),
        BTreeSet::from(["200", "401", "503"].map(str::to_owned))
    );
    assert_eq!(
        response_statuses(&document, "/api/v1/auth/sessions/current", "delete"),
        BTreeSet::from(["204", "401", "503"].map(str::to_owned))
    );

    assert_schema_fields(
        &document,
        "StartLoginRequest",
        serialized_keys(&StartLoginBody {
            code_challenge: "fixture",
        }),
    );
    assert_schema_fields(
        &document,
        "RedeemLoginRequest",
        serialized_keys(&RedeemLoginBody {
            code_verifier: "fixture",
        }),
    );
    assert_schema_fields(
        &document,
        "RefreshRequest",
        serialized_keys(&RefreshSessionBody {
            refresh_credential: "fixture",
        }),
    );
    assert_schema_fields(
        &document,
        "StartLoginResponse",
        round_trip_keys::<StartLoginReply>(START_LOGIN_RESPONSE),
    );
    assert_schema_fields(
        &document,
        "GithubLoginResponse",
        round_trip_keys::<RedeemLoginReply>(REDEEM_LOGIN_RESPONSE),
    );
    assert_schema_fields(
        &document,
        "SessionTokenResponse",
        round_trip_keys::<RefreshSessionReply>(REFRESH_RESPONSE),
    );
}
