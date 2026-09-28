//! Credential-leak scan across the crate's own wire, diagnostic, and error
//! surfaces.
//!
//! A canary placed in every credential position must not appear in a request
//! target, a success or failure diagnostic, or any error rendering. This is
//! the crate-level counterpart to the per-call assertions in the other test
//! modules.

use std::fmt::Write;

use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::support::{TestServer, plain_client, respond, wait};
use crate::containers::{ContainerPageRequest, NodeId};
use crate::credentials::{AccessCredential, LoginVerifier, RefreshCredential};
use crate::error::NativeClientError;

const ACCESS_CANARY: &str = "kta_11111111111111111111111111111111.2222222222222222222222222222222222222222222222222222222222222222";
const REFRESH_CANARY: &str = "ktr_33333333333333333333333333333333.4444444444444444444444444444444444444444444444444444444444444444";
/// The verifier value used by the RFC 7636 Appendix B fixture.
const VERIFIER_CANARY: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CONTAINER_PAGE: &str = include_str!("fixtures/reads/container-page-last.json");
const OPERATION_ID: &str = "018f2e34-0000-7000-8000-0000000000cc";

/// Render every diagnostic surface a value has.
fn diagnostics(value: &impl std::fmt::Debug) -> String {
    format!("{value:?}")
}

/// Assert no canary appears in the rendered text.
fn assert_no_canary(rendered: &str, context: &str) {
    for canary in [ACCESS_CANARY, REFRESH_CANARY, VERIFIER_CANARY] {
        assert!(
            !rendered.contains(canary),
            "{context} leaked a credential: {rendered}"
        );
    }
    // Prefixes alone are also a leak signal.
    for prefix in ["kta_", "ktr_"] {
        assert!(
            !rendered.contains(prefix),
            "{context} leaked a credential prefix: {rendered}"
        );
    }
}

#[test]
fn credential_diagnostics_are_redacted_across_every_type() {
    let mut rendered = String::new();
    let access = AccessCredential::parse(ACCESS_CANARY).unwrap();
    let refresh = RefreshCredential::parse(REFRESH_CANARY).unwrap();
    let verifier = LoginVerifier::parse(VERIFIER_CANARY).unwrap();
    writeln!(rendered, "{access:?} {access}").unwrap();
    writeln!(rendered, "{refresh:?} {refresh}").unwrap();
    writeln!(rendered, "{verifier:?} {verifier}").unwrap();
    assert_no_canary(&rendered, "credential diagnostics");

    // Errors carry no credential either.
    for error in [
        AccessCredential::parse("kta_canary").unwrap_err(),
        RefreshCredential::parse("ktr_canary").unwrap_err(),
        LoginVerifier::parse("short").unwrap_err(),
    ] {
        assert_no_canary(&diagnostics(&error), "credential error");
        assert_no_canary(&error.to_string(), "credential error display");
    }
}

#[test]
fn client_errors_are_credential_free_by_construction() {
    let errors = [
        NativeClientError::InvalidOrigin,
        NativeClientError::InvalidConfiguration,
        NativeClientError::InvalidNodeId,
        NativeClientError::InvalidPagination,
        NativeClientError::Cancelled,
        NativeClientError::DeadlineExceeded,
        NativeClientError::Unavailable,
        NativeClientError::InvalidResponse,
        NativeClientError::InvalidRequest,
        NativeClientError::Unauthorized,
        NativeClientError::Forbidden,
        NativeClientError::NotFound,
        NativeClientError::Conflict,
        NativeClientError::ProviderUnavailable,
        NativeClientError::ServiceUnavailable,
        NativeClientError::AgentUnavailable,
        NativeClientError::AgentTimeout,
        NativeClientError::GatewayTimeout,
        NativeClientError::UnexpectedStatus { status: 503 },
    ];
    for error in errors {
        assert_no_canary(&diagnostics(&error), "client error");
        assert_no_canary(&error.to_string(), "client error display");
    }
}

#[tokio::test]
async fn no_credential_reaches_a_request_target_across_the_surface() {
    let verifier = LoginVerifier::parse(VERIFIER_CANARY).unwrap();
    let attempt_id = Uuid::parse_str("018f2e34-0000-7000-8000-000000000001").unwrap();
    let access = AccessCredential::parse(ACCESS_CANARY).unwrap();
    let refresh = RefreshCredential::parse(REFRESH_CANARY).unwrap();
    let node = NodeId::parse("node-1").unwrap();

    let server = TestServer::plain(vec![
        respond(201, include_str!("fixtures/auth/start-login-response.json")),
        respond(
            200,
            include_str!("fixtures/auth/redeem-login-response.json"),
        ),
        respond(200, include_str!("fixtures/auth/refresh-response.json")),
        respond(204, ""),
        respond(200, CONTAINER_PAGE),
        respond(200, include_str!("fixtures/reads/operation.json")),
    ])
    .await;
    let client = plain_client(&server);

    let _ = client
        .start_login(&verifier, &CancellationToken::new())
        .await;
    let _ = client
        .redeem_login(attempt_id, &verifier, &CancellationToken::new())
        .await;
    let _ = client
        .refresh_session(&refresh, &CancellationToken::new())
        .await;
    let _ = client.logout(&access, &CancellationToken::new()).await;
    let _ = client
        .list_containers(
            &access,
            &node,
            &ContainerPageRequest::default(),
            &CancellationToken::new(),
        )
        .await;
    let _ = client
        .get_operation(
            &access,
            Uuid::parse_str(OPERATION_ID).unwrap(),
            &CancellationToken::new(),
        )
        .await;

    let requests = server.requests();
    assert_eq!(requests.len(), 6, "every covered call reached the fixture");
    for request in &requests {
        assert_no_canary(&request.target, "request target");
        // The access credential is the one legitimate header value; the
        // refresh credential and verifier must never appear in any header.
        for (name, value) in &request.headers {
            if name.eq_ignore_ascii_case("authorization") {
                assert!(value.starts_with("Bearer kta_"), "{name} carries access");
                continue;
            }
            assert_no_canary(value, "request header");
        }
        assert!(
            !request.body.contains(ACCESS_CANARY),
            "access credentials never enter a body"
        );
        if request.target.ends_with("/redeem") {
            // The redemption body is the single place the verifier is allowed.
            assert!(request.body.contains(VERIFIER_CANARY));
        } else {
            assert!(
                !request.body.contains(VERIFIER_CANARY),
                "the verifier must cross only the redemption body"
            );
        }
    }
}

#[tokio::test]
async fn transport_and_decode_failures_stay_credential_free() {
    let access = AccessCredential::parse(ACCESS_CANARY).unwrap();
    let refresh = RefreshCredential::parse(REFRESH_CANARY).unwrap();
    let node = NodeId::parse("node-1").unwrap();
    let operation_id = Uuid::parse_str(OPERATION_ID).unwrap();

    // A non-success status, a malformed success body, and a transport reset
    // each surface a closed error with no credential material.
    let server = TestServer::plain(vec![respond(500, "server exploded: kta_leak")]).await;
    let client = plain_client(&server);
    let errors = [
        client
            .logout(&access, &CancellationToken::new())
            .await
            .unwrap_err(),
        client
            .get_operation(&access, operation_id, &CancellationToken::new())
            .await
            .unwrap_err(),
    ];
    let server = TestServer::plain(vec![respond(200, "not json: ktr_leak")]).await;
    let client = plain_client(&server);
    let errors = [
        errors[0],
        errors[1],
        client
            .list_containers(
                &access,
                &node,
                &ContainerPageRequest::default(),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err(),
        client
            .refresh_session(&refresh, &CancellationToken::new())
            .await
            .unwrap_err(),
    ];
    for error in errors {
        assert_no_canary(&diagnostics(&error), "failure diagnostic");
        assert_no_canary(&error.to_string(), "failure display");
    }
}

#[tokio::test]
async fn a_redirect_response_never_carries_a_credential_forward() {
    let target = TestServer::plain(vec![respond(200, CONTAINER_PAGE)]).await;
    let location = format!("{}/api/v1/nodes/node-1/containers", target.origin());
    let origin = TestServer::plain(vec![super::support::Behavior::RespondWithHeaders {
        status: 307,
        body: String::new(),
        headers: vec![("Location", location)],
    }])
    .await;
    let client = plain_client(&origin);
    let access = AccessCredential::parse(ACCESS_CANARY).unwrap();
    let node = NodeId::parse("node-1").unwrap();

    let error = wait(client.list_containers(
        &access,
        &node,
        &ContainerPageRequest::default(),
        &CancellationToken::new(),
    ))
    .await
    .unwrap_err();

    assert_eq!(error, NativeClientError::UnexpectedStatus { status: 307 });
    assert_eq!(origin.request_count(), 1);
    assert_eq!(target.connection_count(), 0);
    assert_eq!(target.request_count(), 0);
    assert_no_canary(&diagnostics(&error), "redirect diagnostic");
}
