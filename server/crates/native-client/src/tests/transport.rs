//! Transport policy: origins, trust anchors, redirects, bounds, deadlines,
//! and cancellation.

use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

use super::support::{
    Behavior, SHORT_DEADLINE, TEST_TIMEOUT, TestServer, client_for, plain_client, respond,
    test_pki, tls_client, wait,
};
use crate::client::{NativeClient, NativeClientConfig, RetryPolicy, TrustPolicy, validate_origin};
use crate::error::NativeClientError;

const JSON_BODY_LIMIT: usize = 64 * 1024;
const ACCESS_CANARY: &str = "kta_11111111111111111111111111111111.2222222222222222222222222222222222222222222222222222222222222222";

/// Transport-level probe: one raw GET executed through the real exchange
/// path, used where no public method should be involved.
async fn probe(client: &NativeClient, path: &str) -> Result<Vec<u8>, NativeClientError> {
    let url = client.origin.join(path).expect("probe URL");
    let request = client.http.get(url).build().expect("probe request builds");
    client
        .execute(
            request,
            RetryPolicy::Never,
            JSON_BODY_LIMIT,
            &CancellationToken::new(),
        )
        .await
}

fn config_error(config: NativeClientConfig) -> NativeClientError {
    match NativeClient::new(config) {
        Ok(_) => panic!("client must not build from this configuration"),
        Err(error) => error,
    }
}

#[test]
fn origins_require_https_except_for_loopback_development() {
    for allowed in [
        "https://controller.example",
        "https://controller.example/",
        "https://127.0.0.1:5150",
        "https://[::1]:5150",
        "http://localhost:5150",
        "http://127.0.0.1:5150",
        "http://[::1]:5150",
    ] {
        assert!(
            validate_origin(allowed).is_ok(),
            "{allowed} must be accepted"
        );
    }
    for rejected in [
        "http://controller.example",
        "http://10.0.0.1:5150",
        "http://localhost.evil.example",
        "https://user:secret@controller.example",
        "https://controller.example/api",
        "https://controller.example/health",
        "https://controller.example?access=1",
        "https://controller.example#fragment",
        "ftp://controller.example",
        "controller.example",
        "not a url",
        "",
    ] {
        assert_eq!(
            validate_origin(rejected).err(),
            Some(NativeClientError::InvalidOrigin),
            "{rejected} must be rejected"
        );
    }
}

#[test]
fn accepted_origins_expose_one_canonical_value() {
    for (raw, canonical) in [
        ("https://controller.example", "https://controller.example/"),
        ("https://controller.example/", "https://controller.example/"),
        // Host case and the default port are normalized away, so two strings
        // for one origin can never compare unequal across runs.
        ("https://CONTROLLER.example", "https://controller.example/"),
        (
            "https://controller.example:443",
            "https://controller.example/",
        ),
        (
            "https://controller.example:8443",
            "https://controller.example:8443/",
        ),
        ("http://localhost:5150", "http://localhost:5150/"),
        ("https://127.0.0.1:5150", "https://127.0.0.1:5150/"),
        ("https://[::1]:5150", "https://[::1]:5150/"),
    ] {
        let client = NativeClient::new(NativeClientConfig::new(raw)).expect("origin builds");
        assert_eq!(
            client.controller_origin(),
            canonical,
            "{raw} must render as one canonical origin"
        );
    }
}

#[test]
fn zero_bounds_are_rejected_before_any_connection() {
    for mutate in [
        (|config: &mut NativeClientConfig| config.request_deadline = Duration::ZERO)
            as fn(&mut NativeClientConfig),
        |config: &mut NativeClientConfig| config.connect_timeout = Duration::ZERO,
        |config: &mut NativeClientConfig| config.json_body_limit = 0,
        |config: &mut NativeClientConfig| config.collection_body_limit = 0,
        // A deadline the monotonic clock cannot represent must fail here, not
        // panic on the first call.
        |config: &mut NativeClientConfig| config.request_deadline = Duration::MAX,
    ] {
        let mut config = NativeClientConfig::new("https://controller.example");
        mutate(&mut config);
        assert_eq!(
            config_error(config),
            NativeClientError::InvalidConfiguration
        );
    }
}

#[test]
fn malformed_trust_bundles_are_rejected_before_any_connection() {
    for pem in [Vec::new(), b"not a certificate".to_vec()] {
        let mut config = NativeClientConfig::new("https://controller.example");
        config.trust = TrustPolicy::PemBundle(pem);
        assert_eq!(
            config_error(config),
            NativeClientError::InvalidConfiguration
        );
    }
}

#[test]
fn native_client_stores_only_transport_configuration() {
    // Destructuring without `..` is a compile-time proof that the struct has
    // exactly these fields: no credential or session state can hide inside.
    let client = NativeClient::new(NativeClientConfig::new("https://controller.example"))
        .expect("client builds");
    let NativeClient {
        origin,
        http: _,
        limits,
    } = client;
    assert_eq!(origin.as_str(), "https://controller.example/");
    assert!(limits.json_body_limit > 0);
    assert!(limits.collection_body_limit > 0);
    assert!(!limits.request_deadline.is_zero());
}

#[test]
fn authorization_headers_are_marked_sensitive_and_never_enter_the_url() {
    let client = NativeClient::new(NativeClientConfig::new("https://controller.example"))
        .expect("client builds");
    let access = crate::credentials::AccessCredential::parse(ACCESS_CANARY).unwrap();
    let url = client.endpoint("/api/v1/status").expect("endpoint builds");
    let request = client
        .authorized_request(reqwest::Method::DELETE, url, &access)
        .expect("request builds");

    let header = request
        .headers()
        .get(reqwest::header::AUTHORIZATION)
        .expect("authorization header present");
    assert!(header.is_sensitive(), "credentials must never be logged");
    let expected = format!("Bearer {ACCESS_CANARY}");
    assert_eq!(header.to_str().unwrap(), expected);
    assert!(!request.url().as_str().contains("kta_"));
    assert_eq!(request.url().path(), "/api/v1/status");
    assert!(request.url().query().is_none());
}

#[tokio::test]
async fn webpki_policy_still_reaches_loopback_plaintext_development_origins() {
    let server = TestServer::plain(vec![respond(200, "{}")]).await;
    let client = plain_client(&server);

    let body = wait(probe(&client, "/health/live"))
        .await
        .expect("loopback plaintext development origins work");
    assert_eq!(body, b"{}");
}

#[tokio::test]
async fn private_ca_is_rejected_under_webpki_and_accepted_with_a_bundle() {
    let pki = test_pki();
    let server = TestServer::tls(
        pki.server_config.clone(),
        vec![respond(200, "{}"), respond(200, "{}")],
    )
    .await;

    let webpki_client = client_for(&server.origin(), TrustPolicy::WebPki, None);
    assert_eq!(
        probe(&webpki_client, "/health/live").await.err(),
        Some(NativeClientError::Unavailable)
    );
    assert_eq!(server.connection_count(), 1);
    assert_eq!(
        server.request_count(),
        0,
        "a rejected handshake never reaches a handler"
    );

    let trusted_client = tls_client(&server, &pki.ca_pem);
    let body = wait(probe(&trusted_client, "/health/live"))
        .await
        .expect("trusted CA reaches the fixture");
    assert_eq!(body, b"{}");
    assert_eq!(server.request_count(), 1);
}

#[tokio::test]
async fn wrong_hostname_is_rejected_even_with_the_ca_trusted() {
    let pki = test_pki();
    let server = TestServer::tls(pki.wrong_name_config.clone(), vec![respond(200, "{}")]).await;

    let client = tls_client(&server, &pki.ca_pem);
    assert_eq!(
        probe(&client, "/health/live").await.err(),
        Some(NativeClientError::Unavailable)
    );
    assert_eq!(server.connection_count(), 1);
    assert_eq!(server.request_count(), 0);
}

#[tokio::test]
async fn a_bundle_with_several_cas_trusts_every_listed_anchor() {
    let pki = test_pki();
    let mut bundle = pki.other_ca_pem.clone();
    bundle.extend_from_slice(&pki.ca_pem);
    let server = TestServer::tls(pki.server_config.clone(), vec![respond(200, "{}")]).await;

    let client = client_for(&server.origin(), TrustPolicy::PemBundle(bundle), None);
    let body = wait(probe(&client, "/health/live"))
        .await
        .expect("listed CA anchors are trusted");
    assert_eq!(body, b"{}");
}

#[tokio::test]
async fn redirects_are_never_followed_and_the_target_receives_nothing() {
    let target = TestServer::plain(vec![respond(200, "{}")]).await;
    let location = format!("{}/post-login", target.origin());
    let origin = TestServer::plain(vec![Behavior::RespondWithHeaders {
        status: 302,
        body: String::new(),
        headers: vec![("Location", location)],
    }])
    .await;

    let client = plain_client(&origin);
    // The read methods are the retryable class; a redirect response must not
    // be retried or followed even there.
    let url = client.origin.join("/health/live").expect("redirect URL");
    let request = client.http.get(url).build().expect("redirect request");
    let error = client
        .execute(
            request,
            RetryPolicy::Once,
            JSON_BODY_LIMIT,
            &CancellationToken::new(),
        )
        .await
        .expect_err("a redirect is not a success");
    assert_eq!(error, NativeClientError::UnexpectedStatus { status: 302 });
    assert_eq!(origin.connection_count(), 1);
    assert_eq!(target.connection_count(), 0);
    assert_eq!(target.request_count(), 0);
}

#[tokio::test]
async fn an_oversized_declared_body_is_rejected_without_reading_it() {
    let server = TestServer::plain(vec![Behavior::Truncated {
        declared_length: JSON_BODY_LIMIT + 1,
        body: String::new(),
    }])
    .await;

    let client = plain_client(&server);
    assert_eq!(
        probe(&client, "/health/live").await.err(),
        Some(NativeClientError::InvalidResponse)
    );
    assert_eq!(server.connection_count(), 1);
}

#[tokio::test]
async fn chunked_bodies_are_streamed_and_bounded() {
    let server = TestServer::plain(vec![
        Behavior::Chunked {
            status: 200,
            body: "ok".to_owned(),
        },
        Behavior::Chunked {
            status: 200,
            body: "x".repeat(JSON_BODY_LIMIT + 1),
        },
    ])
    .await;

    let client = plain_client(&server);
    let body = wait(probe(&client, "/health/live"))
        .await
        .expect("a bounded chunked body is accepted");
    assert_eq!(body, b"ok");

    assert_eq!(
        probe(&client, "/health/live").await.err(),
        Some(NativeClientError::InvalidResponse)
    );
    assert_eq!(server.connection_count(), 2);
}

#[tokio::test]
async fn deadline_expiry_cancels_the_in_flight_call() {
    let server = TestServer::plain(vec![Behavior::Stall]).await;
    let client = client_for(&server.origin(), TrustPolicy::WebPki, Some(SHORT_DEADLINE));

    let started = Instant::now();
    let error = probe(&client, "/health/live")
        .await
        .expect_err("a stalled exchange expires");
    let elapsed = started.elapsed();

    assert_eq!(error, NativeClientError::DeadlineExceeded);
    assert!(
        elapsed >= SHORT_DEADLINE,
        "deadline fired early: {elapsed:?}"
    );
    assert!(
        elapsed < TEST_TIMEOUT,
        "deadline was not honored: {elapsed:?}"
    );
    wait_for_peer_close(&server).await;
}

#[tokio::test]
async fn cancellation_returns_promptly_and_closes_the_connection() {
    let server = TestServer::plain(vec![Behavior::Stall]).await;
    let client = plain_client(&server);
    let cancellation = CancellationToken::new();

    let url = client.origin.join("/health/live").expect("cancel URL");
    let request = client.http.get(url).build().expect("cancel request");
    let mut call =
        Box::pin(client.execute(request, RetryPolicy::Never, JSON_BODY_LIMIT, &cancellation));

    tokio::select! {
        result = &mut call => panic!("call completed before cancellation: {result:?}"),
        () = server.wait_for_request() => cancellation.cancel(),
    }
    let error = wait(call).await.expect_err("cancellation is not success");
    assert_eq!(error, NativeClientError::Cancelled);
    wait_for_peer_close(&server).await;
}

async fn wait_for_peer_close(server: &TestServer) {
    wait(server.wait_for_peer_close()).await;
}
