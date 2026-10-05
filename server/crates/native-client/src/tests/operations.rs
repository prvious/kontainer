//! Operation cancellation and mutation admission contracts.

use serde_json::json;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::support::{TestServer, plain_client, respond};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;
use crate::operations::{IdempotencyKey, OperationState};

const ACCESS: &str = "kta_11111111111111111111111111111111.2222222222222222222222222222222222222222222222222222222222222222";
const OPERATION_ID: &str = "018f2e34-0000-7000-8000-0000000000cc";
const KEY: &str = "018f2e34-0000-7000-8000-0000000000aa";

fn access() -> AccessCredential {
    AccessCredential::parse(ACCESS).unwrap()
}

fn operation_id() -> Uuid {
    Uuid::parse_str(OPERATION_ID).unwrap()
}

#[tokio::test]
async fn cancel_posts_once_and_maps_not_cancellable_to_conflict() {
    let mut cancelled: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/reads/operation.json")).unwrap();
    cancelled["state"] = json!("cancelled");
    cancelled["progress"] = json!(null);
    cancelled["finishedAt"] = json!("2026-09-19T06:52:00.000Z");
    let server = TestServer::plain(vec![
        respond(202, &cancelled.to_string()),
        respond(409, r#"{"error":"not_cancellable"}"#),
    ])
    .await;
    let client = plain_client(&server);
    let snapshot = client
        .cancel_operation(&access(), operation_id(), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(snapshot.state, OperationState::Cancelled);
    assert!(snapshot.finished_at.is_some());
    assert_eq!(
        client
            .cancel_operation(&access(), operation_id(), &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::Conflict)
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|request| request.method == "POST"
        && request.target == format!("/api/v1/operations/{OPERATION_ID}/cancel")));
}

#[tokio::test]
async fn admission_sends_the_key_and_body_and_reads_replays_and_conflicts() {
    let server = TestServer::plain(vec![
        respond(
            202,
            &json!({"operationId": OPERATION_ID, "replayed": false}).to_string(),
        ),
        respond(
            202,
            &json!({"operationId": OPERATION_ID, "replayed": true}).to_string(),
        ),
        respond(409, r#"{"error":"idempotency_conflict"}"#),
        respond(
            202,
            &json!({"operationId": "not-a-uuid", "replayed": false}).to_string(),
        ),
    ])
    .await;
    let client = plain_client(&server);
    let key = IdempotencyKey::parse(KEY).unwrap();
    let body = json!({"steps": 2});
    let first = client
        .admit(
            &access(),
            "/api/v1/operations/noop",
            &key,
            &body,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(first.operation_id, operation_id());
    assert!(!first.replayed);
    let replay = client
        .admit(
            &access(),
            "/api/v1/operations/noop",
            &key,
            &body,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(
        client
            .admit(
                &access(),
                "/api/v1/operations/noop",
                &key,
                &json!({"steps": 3}),
                &CancellationToken::new()
            )
            .await
            .err(),
        Some(NativeClientError::Conflict)
    );
    assert_eq!(
        client
            .admit(
                &access(),
                "/api/v1/operations/noop",
                &key,
                &body,
                &CancellationToken::new()
            )
            .await
            .err(),
        Some(NativeClientError::InvalidResponse)
    );
    let request = &server.requests()[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.header("idempotency-key"), Some(KEY));
    assert_eq!(request.header("content-type"), Some("application/json"));
    assert_eq!(request.body, r#"{"steps":2}"#);

    for bad in ["short", &"k".repeat(65), "has spaces in the key"] {
        assert_eq!(
            IdempotencyKey::parse(bad).err(),
            Some(NativeClientError::InvalidRequest)
        );
    }
}
