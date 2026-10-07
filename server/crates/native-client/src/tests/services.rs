//! Service list, detail, and action contracts, including the quorum and version refusals.

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::support::{
    TestServer, assert_openapi_snapshot, assert_schema_fields, covered_contract, openapi_artifact,
    plain_client, respond, response_statuses, round_trip_keys,
};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;
use crate::operations::IdempotencyKey;
use crate::services::{ServiceAction, ServiceDetailBody, ServiceId, ServiceListBody, ServiceMode};

const SERVICE_LIST: &str = include_str!("fixtures/services/service-list.json");
const ACCESS: &str = "kta_11111111111111111111111111111111.2222222222222222222222222222222222222222222222222222222222222222";
const OPERATION_ID: &str = "018f2e34-0000-7000-8000-0000000000cc";
const KEY: &str = "018f2e34-0000-7000-8000-0000000000aa";
const WEB: &str = "webappweb0000000000000000";

fn access() -> AccessCredential {
    AccessCredential::parse(ACCESS).unwrap()
}

fn key() -> IdempotencyKey {
    IdempotencyKey::parse(KEY).unwrap()
}

#[tokio::test]
async fn services_list_and_detail_parse_counts_and_the_latest_error() {
    let detail = json!({
        "latestError": "no suitable node (insufficient memory on 3 nodes)"
    })
    .to_string();
    let server = TestServer::plain(vec![
        respond(200, SERVICE_LIST),
        respond(200, SERVICE_LIST),
        respond(200, &detail),
    ])
    .await;
    let client = plain_client(&server);
    let services = client
        .list_services(&access(), None, &CancellationToken::new())
        .await
        .unwrap();
    let web = &services.items[0];
    assert_eq!(
        (
            web.mode,
            web.running_replicas,
            web.desired_replicas,
            web.version
        ),
        (ServiceMode::Replicated, 2, 3, 7)
    );
    assert_eq!(web.image, "nginx:alpine");
    assert_eq!(services.items[1].mode, ServiceMode::Global);
    client
        .list_services(&access(), Some("webapp"), &CancellationToken::new())
        .await
        .unwrap();
    let detail = client
        .get_service(
            &access(),
            &ServiceId::parse(WEB).unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        detail.latest_error.as_deref(),
        Some("no suitable node (insufficient memory on 3 nodes)")
    );
    let targets: Vec<_> = server
        .requests()
        .into_iter()
        .map(|request| request.target)
        .collect();
    assert_eq!(
        targets,
        vec![
            "/api/v1/services".to_owned(),
            "/api/v1/services?stack=webapp".to_owned(),
            format!("/api/v1/services/{WEB}")
        ]
    );
}

#[tokio::test]
async fn quorum_and_manager_refusals_are_their_own_errors() {
    let server = TestServer::plain(vec![
        respond(
            503,
            r#"{"error":"quorum_unavailable","retryAfterSeconds":10}"#,
        ),
        respond(
            503,
            r#"{"error":"quorum_unavailable","retryAfterSeconds":10}"#,
        ),
        respond(503, r#"{"error":"not_a_manager"}"#),
        respond(409, r#"{"error":"version_conflict"}"#),
        respond(409, r#"{"error":"idempotency_conflict"}"#),
    ])
    .await;
    let client = plain_client(&server);
    assert_eq!(
        client
            .list_services(&access(), None, &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::QuorumUnavailable {
            retry_after_seconds: 10
        })
    );
    let id = ServiceId::parse(WEB).unwrap();
    let (access, key, token) = (access(), key(), CancellationToken::new());
    let action = |action| client.service_action(&access, &id, action, 7, &key, &token);
    assert_eq!(
        action(ServiceAction::Restart).await.err(),
        Some(NativeClientError::QuorumUnavailable {
            retry_after_seconds: 10
        })
    );
    assert_eq!(
        action(ServiceAction::Rollback).await.err(),
        Some(NativeClientError::NotAManager)
    );
    assert_eq!(
        action(ServiceAction::Scale { replicas: 2 }).await.err(),
        Some(NativeClientError::VersionConflict)
    );
    assert_eq!(
        action(ServiceAction::Scale { replicas: 2 }).await.err(),
        Some(NativeClientError::Conflict)
    );
}

#[tokio::test]
async fn actions_send_the_version_and_validate_locally() {
    let admitted = json!({"operationId": OPERATION_ID, "replayed": false}).to_string();
    let server = TestServer::plain(vec![respond(202, &admitted), respond(202, &admitted)]).await;
    let client = plain_client(&server);
    let id = ServiceId::parse(WEB).unwrap();
    client
        .service_action(
            &access(),
            &id,
            ServiceAction::Scale { replicas: 2 },
            7,
            &key(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    client
        .service_action(
            &access(),
            &id,
            ServiceAction::Restart,
            8,
            &key(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    let requests = server.requests();
    assert_eq!(
        requests[0].target,
        format!("/api/v1/services/{WEB}/actions/scale")
    );
    assert_eq!(requests[0].body, r#"{"version":7,"replicas":2}"#);
    assert_eq!(
        requests[1].target,
        format!("/api/v1/services/{WEB}/actions/restart")
    );
    assert_eq!(requests[1].body, r#"{"version":8}"#);
    assert_eq!(
        client
            .service_action(
                &access(),
                &id,
                ServiceAction::Scale { replicas: 1001 },
                7,
                &key(),
                &CancellationToken::new()
            )
            .await
            .err(),
        Some(NativeClientError::InvalidRequest)
    );
    for bad in ["short", "WEBAPPWEB0000000000000000"] {
        assert_eq!(
            ServiceId::parse(bad).err(),
            Some(NativeClientError::InvalidRequest)
        );
    }
    assert_eq!(server.request_count(), 2);
}

const SERVICE_OPENAPI_PATHS: [(&str, &str); 3] = [
    ("/api/v1/services", "get"),
    ("/api/v1/services/{serviceId}", "get"),
    ("/api/v1/services/{serviceId}/actions/{action}", "post"),
];
const SERVICE_OPENAPI_SCHEMAS: [&str; 5] = [
    "ServiceListResponse",
    "ServiceResponse",
    "ServiceDetailResponse",
    "QuorumUnavailableResponse",
    "ServiceActionRequest",
];

#[test]
fn openapi_services_contract_matches_the_checked_artifact() {
    let document = openapi_artifact();
    assert_openapi_snapshot(
        "services/openapi-contract.json",
        &covered_contract(&document, &SERVICE_OPENAPI_PATHS, &SERVICE_OPENAPI_SCHEMAS),
    );
    let statuses = response_statuses(
        &document,
        "/api/v1/services/{serviceId}/actions/{action}",
        "post",
    );
    assert!(statuses.contains("202") && statuses.contains("409") && statuses.contains("503"));
    assert_schema_fields(
        &document,
        "ServiceListResponse",
        round_trip_keys::<ServiceListBody>(SERVICE_LIST),
    );
    assert_schema_fields(
        &document,
        "ServiceDetailResponse",
        round_trip_keys::<ServiceDetailBody>(r#"{"latestError":null}"#),
    );
    let list: serde_json::Value = serde_json::from_str(SERVICE_LIST).unwrap();
    assert_schema_fields(
        &document,
        "ServiceResponse",
        list["items"][0]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect(),
    );
}
