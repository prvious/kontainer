//! Retry classification: exactly one retry, only for the two read-only GETs,
//! only for allowlisted pre-response transients, always under the original
//! deadline.

use std::io::ErrorKind;
use std::time::Instant;

use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::support::{
    Behavior, SHORT_DEADLINE, TestServer, client_for, plain_client, respond, test_pki, tls_client,
    wait,
};
use crate::client::TrustPolicy;
use crate::containers::{ContainerPageRequest, NodeId};
use crate::credentials::AccessCredential;
use crate::error::{NativeClientError, retryable_cause_chain};
use crate::operations::OperationState;

const CONTAINER_PAGE: &str = include_str!("fixtures/reads/container-page-last.json");
const OPERATION: &str = include_str!("fixtures/reads/operation-uncertain.json");
const ACCESS_CANARY: &str = "kta_11111111111111111111111111111111.2222222222222222222222222222222222222222222222222222222222222222";
const OPERATION_ID: &str = "018f2e34-0000-7000-8000-0000000000cc";

fn access() -> AccessCredential {
    AccessCredential::parse(ACCESS_CANARY).unwrap()
}

fn node() -> NodeId {
    NodeId::parse("node-1").unwrap()
}

/// A support error whose source chain is what the classifier walks.
#[derive(Debug)]
struct Chain {
    inner: Box<dyn std::error::Error + 'static>,
}

impl std::fmt::Display for Chain {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("transport failure")
    }
}

impl std::error::Error for Chain {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.inner.as_ref())
    }
}

#[test]
fn retry_classification_allows_only_allowlisted_io_causes() {
    for kind in [
        ErrorKind::ConnectionReset,
        ErrorKind::ConnectionRefused,
        ErrorKind::ConnectionAborted,
        ErrorKind::BrokenPipe,
        ErrorKind::UnexpectedEof,
        ErrorKind::NotConnected,
    ] {
        assert!(
            retryable_cause_chain(&std::io::Error::new(kind, "fixture")),
            "{kind:?} is a transient pre-response failure"
        );
    }
    for kind in [
        ErrorKind::InvalidData,
        ErrorKind::PermissionDenied,
        ErrorKind::TimedOut,
        ErrorKind::Other,
    ] {
        assert!(
            !retryable_cause_chain(&std::io::Error::new(kind, "fixture")),
            "{kind:?} must not retry"
        );
    }
}

#[test]
fn retry_classification_walks_wrapped_causes() {
    // What reqwest produces: a library error whose source is the I/O error.
    let wrapped = Chain {
        inner: Box::new(std::io::Error::new(ErrorKind::ConnectionReset, "fixture")),
    };
    assert!(retryable_cause_chain(&wrapped));

    // An I/O error that only wraps another I/O error is not positively
    // identified and fails closed.
    let io_over_io =
        std::io::Error::other(std::io::Error::new(ErrorKind::ConnectionReset, "fixture"));
    assert!(!retryable_cause_chain(&io_over_io));
}

#[test]
fn retry_classification_rejects_tls_and_unknown_causes() {
    // Certificate validation errors carry no allowlisted I/O cause, so they
    // never retry. The unknown-CA wire test pins the same behavior end to
    // end with exactly one connection attempt.
    let tls = rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer);
    assert!(!retryable_cause_chain(&tls));
    assert!(!retryable_cause_chain(&Chain {
        inner: Box::new(tls),
    }));
}

#[tokio::test]
async fn list_containers_retries_once_after_a_connection_reset() {
    let server = TestServer::plain(vec![Behavior::Reset, respond(200, CONTAINER_PAGE)]).await;
    let client = plain_client(&server);

    let page = wait(client.list_containers(
        &access(),
        &node(),
        &ContainerPageRequest::default(),
        &CancellationToken::new(),
    ))
    .await
    .expect("the retry succeeds");

    assert_eq!(page.items.len(), 1);
    assert_eq!(server.connection_count(), 2);
    let requests = server.requests();
    assert_eq!(requests.len(), 2, "both attempts carried the same request");
    assert_eq!(requests[0].target, requests[1].target);
    assert_eq!(requests[0].method, "GET");
}

#[tokio::test]
async fn get_operation_retries_once_after_a_connection_reset() {
    let server = TestServer::plain(vec![Behavior::Reset, respond(200, OPERATION)]).await;
    let client = plain_client(&server);

    let snapshot = wait(client.get_operation(
        &access(),
        Uuid::parse_str(OPERATION_ID).unwrap(),
        &CancellationToken::new(),
    ))
    .await
    .expect("the retry succeeds");

    assert_eq!(snapshot.state, OperationState::Uncertain);
    assert_eq!(server.connection_count(), 2);
}

#[tokio::test]
async fn a_second_transient_failure_stops_after_one_retry() {
    let server = TestServer::plain(vec![Behavior::Reset, Behavior::Reset]).await;
    let client = plain_client(&server);

    assert_eq!(
        client
            .list_containers(
                &access(),
                &node(),
                &ContainerPageRequest::default(),
                &CancellationToken::new(),
            )
            .await
            .err(),
        Some(NativeClientError::Unavailable)
    );
    assert_eq!(server.connection_count(), 2, "exactly one retry");

    let server = TestServer::plain(vec![Behavior::Reset, Behavior::Reset]).await;
    let client = plain_client(&server);
    assert_eq!(
        client
            .get_operation(
                &access(),
                Uuid::parse_str(OPERATION_ID).unwrap(),
                &CancellationToken::new(),
            )
            .await
            .err(),
        Some(NativeClientError::Unavailable)
    );
    assert_eq!(server.connection_count(), 2, "exactly one retry");
}

#[tokio::test]
async fn http_error_responses_are_never_retried() {
    let server = TestServer::plain(vec![respond(503, r#"{"error":"unavailable"}"#)]).await;
    let client = plain_client(&server);

    assert_eq!(
        client
            .get_operation(
                &access(),
                Uuid::parse_str(OPERATION_ID).unwrap(),
                &CancellationToken::new(),
            )
            .await
            .err(),
        Some(NativeClientError::ServiceUnavailable)
    );
    assert_eq!(server.connection_count(), 1);
}

#[tokio::test]
async fn tls_identity_failures_are_never_retried() {
    let pki = test_pki();
    let server = TestServer::tls(
        pki.server_config.clone(),
        vec![respond(200, CONTAINER_PAGE)],
    )
    .await;
    let client = client_for(
        &server.origin(),
        TrustPolicy::PemBundle(pki.other_ca_pem.clone()),
        None,
    );

    assert_eq!(
        client
            .list_containers(
                &access(),
                &node(),
                &ContainerPageRequest::default(),
                &CancellationToken::new(),
            )
            .await
            .err(),
        Some(NativeClientError::Unavailable)
    );
    assert_eq!(server.connection_count(), 1);
    assert_eq!(
        server.request_count(),
        0,
        "a rejected handshake is never retried"
    );

    // The right CA retries nothing and reads the page.
    let server = TestServer::tls(
        pki.server_config.clone(),
        vec![respond(200, CONTAINER_PAGE)],
    )
    .await;
    let client = tls_client(&server, &pki.ca_pem);
    wait(client.list_containers(
        &access(),
        &node(),
        &ContainerPageRequest::default(),
        &CancellationToken::new(),
    ))
    .await
    .expect("trusted CA reads the page");
    assert_eq!(server.connection_count(), 1);
}

#[tokio::test]
async fn a_truncated_response_body_is_never_retried() {
    let server = TestServer::plain(vec![Behavior::Truncated {
        declared_length: 512,
        body: "{".to_owned(),
    }])
    .await;
    let client = plain_client(&server);

    assert_eq!(
        client
            .list_containers(
                &access(),
                &node(),
                &ContainerPageRequest::default(),
                &CancellationToken::new(),
            )
            .await
            .err(),
        Some(NativeClientError::Unavailable)
    );
    assert_eq!(
        server.connection_count(),
        1,
        "a received response never retries"
    );
}

#[tokio::test]
async fn the_retry_shares_the_original_deadline() {
    // The first attempt burns most of the budget before failing; a per-attempt
    // budget would let the retry run a second full deadline and finish near
    // twice the configured bound.
    let server = TestServer::plain(vec![
        Behavior::DelayedReset {
            delay: SHORT_DEADLINE * 3 / 5,
        },
        Behavior::Stall,
    ])
    .await;
    let client = client_for(&server.origin(), TrustPolicy::WebPki, Some(SHORT_DEADLINE));

    let started = Instant::now();
    let error = client
        .list_containers(
            &access(),
            &node(),
            &ContainerPageRequest::default(),
            &CancellationToken::new(),
        )
        .await
        .expect_err("the stalled retry expires");
    let elapsed = started.elapsed();

    assert_eq!(error, NativeClientError::DeadlineExceeded);
    assert_eq!(server.connection_count(), 2, "the retry was attempted");
    assert!(
        elapsed >= SHORT_DEADLINE,
        "the deadline expired early: {elapsed:?}"
    );
    assert!(
        elapsed < SHORT_DEADLINE * 3 / 2,
        "the retry extended the original deadline: {elapsed:?}"
    );
    wait(server.wait_for_peer_close()).await;
}
