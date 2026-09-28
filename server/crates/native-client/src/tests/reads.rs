//! Node membership, samples, settings, container inventory, and operation polling contracts.

use std::collections::BTreeSet;

use chrono::SecondsFormat;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::support::{
    TestServer, assert_openapi_snapshot, assert_schema_fields, covered_contract, openapi_artifact,
    plain_client, respond, response_statuses, round_trip_keys, serialized_keys, wait,
};
use crate::client::{NativeClientConfig, TrustPolicy};
use crate::containers::{
    ContainerItemBody, ContainerPageBody, ContainerPageRequest, ContainerState, NodeId,
};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;
use crate::nodes::{NodeListBody, NodeStatus, SampleSeriesBody};
use crate::operations::{OperationBody, OperationState};

const CONTAINER_PAGE: &str = include_str!("fixtures/reads/container-page.json");
const CONTAINER_PAGE_LAST: &str = include_str!("fixtures/reads/container-page-last.json");
const CONTAINER_PAGE_FOREIGN_NODE: &str =
    include_str!("fixtures/reads/container-page-foreign-node.json");
const CONTAINER_PAGE_BAD_CURSOR: &str =
    include_str!("fixtures/reads/container-page-bad-cursor.json");
const NODES: &str = include_str!("fixtures/reads/nodes.json");
const CLUSTER_SAMPLES: &str = include_str!("fixtures/reads/cluster-samples.json");
const OPERATION: &str = include_str!("fixtures/reads/operation.json");
const OPERATION_UNCERTAIN: &str = include_str!("fixtures/reads/operation-uncertain.json");
const OPERATION_INCONSISTENT: &str = include_str!("fixtures/reads/operation-inconsistent.json");
const OPERATION_UNKNOWN_STATE: &str = include_str!("fixtures/reads/operation-unknown-state.json");

const ACCESS_CANARY: &str = "kta_11111111111111111111111111111111.2222222222222222222222222222222222222222222222222222222222222222";
const FIRST_CONTAINER_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CURSOR: &str = "v1:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const OPERATION_ID: &str = "018f2e34-0000-7000-8000-0000000000cc";

fn access() -> AccessCredential {
    AccessCredential::parse(ACCESS_CANARY).unwrap()
}

fn node() -> NodeId {
    NodeId::parse("node-1").unwrap()
}

#[tokio::test]
async fn node_membership_is_strict_and_authenticated() {
    let server = TestServer::plain(vec![
        respond(200, NODES),
        respond(401, r#"{"error":"unauthorized"}"#),
    ])
    .await;
    let client = plain_client(&server);
    let list = client
        .list_nodes(&access(), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(list.items.len(), 2);
    let manager = &list.items[0];
    assert_eq!(manager.id, NodeId::parse("manager").unwrap());
    assert_eq!(manager.status, NodeStatus::Online);
    assert_eq!(manager.cpu_cores, 4);
    let sample = manager.latest_sample.unwrap();
    assert_eq!(sample.cpu_percent, 12.5);
    assert_eq!(sample.disk_used_bytes, Some(5 << 30));
    assert_eq!(sample.net_tx_bytes_per_sec, None);
    assert_eq!(list.items[1].status, NodeStatus::Offline);
    assert!(list.items[1].latest_sample.is_none());
    assert_eq!(
        client
            .list_nodes(&access(), &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::Unauthorized)
    );
    for request in server.requests() {
        assert_eq!(request.method, "GET");
        assert_eq!(request.target, "/api/v1/nodes");
        assert!(
            request
                .header("authorization")
                .unwrap()
                .starts_with("Bearer ")
        );
        assert!(!request.target.contains(ACCESS_CANARY));
    }

    let valid: serde_json::Value = serde_json::from_str(NODES).unwrap();
    let mut extra = valid.clone();
    extra["items"][0]["secret"] = json!("canary");
    let mut bad_id = valid.clone();
    bad_id["items"][0]["id"] = json!("../bad");
    let mut bad_time = valid.clone();
    bad_time["items"][0]["lastCheckInAt"] = json!("2026-09-28T12:00:00Z");
    let mut bad_status = valid.clone();
    bad_status["items"][0]["status"] = json!("unknown");
    let mut negative_cpu = valid;
    negative_cpu["items"][0]["latestSample"]["cpuPercent"] = json!(-1.0);
    for body in [extra, bad_id, bad_time, bad_status, negative_cpu] {
        let server = TestServer::plain(vec![respond(200, &body.to_string())]).await;
        assert_eq!(
            plain_client(&server)
                .list_nodes(&access(), &CancellationToken::new())
                .await
                .err(),
            Some(NativeClientError::InvalidResponse),
            "{body}"
        );
    }
}

#[tokio::test]
async fn sample_series_keep_null_buckets_and_validate_ranges_locally() {
    let server = TestServer::plain(vec![
        respond(200, CLUSTER_SAMPLES),
        respond(
            200,
            &CLUSTER_SAMPLES.replace(r#""window": "5m""#, r#""window": "1h""#),
        ),
        respond(404, r#"{"error":"not_found"}"#),
    ])
    .await;
    let client = plain_client(&server);
    let series = client
        .cluster_samples(&access(), "5m", "60s", &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(series.points.len(), 5);
    assert_eq!(series.points[0].cpu_percent, None, "a gap stays null");
    assert_eq!(series.points[0].nodes, 0);
    assert_eq!(series.points[1].cpu_percent, Some(30.0));
    assert_eq!(series.points[1].nodes, 2);
    assert_eq!(
        client
            .node_samples(&access(), &node(), "5m", "60s", &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::InvalidResponse),
        "a series for another window is rejected"
    );
    assert_eq!(
        client
            .node_samples(&access(), &node(), "1h", "60s", &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::NotFound)
    );
    let targets: Vec<_> = server.requests().into_iter().map(|r| r.target).collect();
    assert_eq!(
        targets,
        vec![
            "/api/v1/cluster/samples?window=5m&bucket=60s",
            "/api/v1/nodes/node-1/samples?window=5m&bucket=60s",
            "/api/v1/nodes/node-1/samples?window=1h&bucket=60s",
        ]
    );

    for (window, bucket) in [
        ("2h", "60s"),
        ("1h", "0s"),
        ("1h", "60"),
        ("1h", "1d"),
        ("1h", ""),
    ] {
        assert_eq!(
            client
                .cluster_samples(&access(), window, bucket, &CancellationToken::new())
                .await
                .err(),
            Some(NativeClientError::InvalidRequest),
            "{window}/{bucket}"
        );
    }
    assert_eq!(server.request_count(), 3, "local validation sends nothing");

    let mut unordered: serde_json::Value = serde_json::from_str(CLUSTER_SAMPLES).unwrap();
    unordered["points"].as_array_mut().unwrap().swap(0, 1);
    let server = TestServer::plain(vec![respond(200, &unordered.to_string())]).await;
    assert_eq!(
        plain_client(&server)
            .cluster_samples(&access(), "5m", "60s", &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::InvalidResponse)
    );
}

#[tokio::test]
async fn settings_read_and_patch_retention_with_bounds() {
    let server = TestServer::plain(vec![
        respond(200, r#"{"metricsRetentionDays":7}"#),
        respond(200, r#"{"metricsRetentionDays":30}"#),
        respond(422, r#"{"error":"retention_out_of_range"}"#),
        respond(200, r#"{"metricsRetentionDays":0}"#),
    ])
    .await;
    let client = plain_client(&server);
    assert_eq!(
        client
            .get_settings(&access(), &CancellationToken::new())
            .await
            .unwrap()
            .metrics_retention_days,
        7
    );
    assert_eq!(
        client
            .update_settings(&access(), 30, &CancellationToken::new())
            .await
            .unwrap()
            .metrics_retention_days,
        30
    );
    assert_eq!(
        client
            .update_settings(&access(), 30, &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::InvalidRequest)
    );
    assert_eq!(
        client
            .get_settings(&access(), &CancellationToken::new())
            .await
            .err(),
        Some(NativeClientError::InvalidResponse),
        "out-of-range values from the controller are rejected"
    );
    for days in [0, 366] {
        assert_eq!(
            client
                .update_settings(&access(), days, &CancellationToken::new())
                .await
                .err(),
            Some(NativeClientError::InvalidRequest)
        );
    }
    let requests = server.requests();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[1].method, "PATCH");
    assert_eq!(requests[1].target, "/api/v1/settings");
    assert_eq!(requests[1].header("content-type"), Some("application/json"));
    assert_eq!(requests[1].body, r#"{"metricsRetentionDays":30}"#);
}

#[tokio::test]
async fn inventory_error_envelopes_distinguish_agent_from_controller_without_leaking_body() {
    for (status, body, expected) in [
        (
            400,
            r#"{"error":"invalid_pagination"}"#,
            NativeClientError::InvalidPagination,
        ),
        (
            503,
            r#"{"error":"agent_unavailable"}"#,
            NativeClientError::AgentUnavailable,
        ),
        (
            503,
            r#"{"error":"unavailable"}"#,
            NativeClientError::ServiceUnavailable,
        ),
        (
            504,
            r#"{"error":"agent_timeout"}"#,
            NativeClientError::AgentTimeout,
        ),
        (
            504,
            r#"{"error":"proxy_canary"}"#,
            NativeClientError::GatewayTimeout,
        ),
        (
            503,
            r#"{"error":"agent_unavailable","identity":"secret-canary"}"#,
            NativeClientError::ServiceUnavailable,
        ),
    ] {
        let server = TestServer::plain(vec![respond(status, body)]).await;
        let error = plain_client(&server)
            .list_containers(
                &access(),
                &node(),
                &ContainerPageRequest::default(),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?} {error}").contains("canary"));
        assert_eq!(server.request_count(), 1);
    }
}

#[test]
fn node_ids_mirror_the_controller_identity_rules() {
    for valid in ["node-1", "node_1", "a.b-c", "NODE", &"a".repeat(128)] {
        assert!(NodeId::parse(valid).is_ok(), "{valid} must be accepted");
    }
    for invalid in [
        "",
        ".",
        "..",
        "with space",
        "slash/segment",
        "colon:segment",
        &"a".repeat(129),
    ] {
        assert_eq!(
            NodeId::parse(invalid).err(),
            Some(NativeClientError::InvalidNodeId),
            "{invalid} must be rejected"
        );
    }
    assert_eq!(node().as_str(), "node-1");
    assert_eq!(node().to_string(), "node-1");
}

#[tokio::test]
async fn list_containers_sends_bounded_pagination_and_parses_the_page() {
    let server = TestServer::plain(vec![
        respond(200, CONTAINER_PAGE),
        respond(200, CONTAINER_PAGE),
    ])
    .await;
    let client = plain_client(&server);
    let page_request = ContainerPageRequest {
        limit: 2,
        cursor: Some(CURSOR.to_owned()),
    };

    let page =
        wait(client.list_containers(&access(), &node(), &page_request, &CancellationToken::new()))
            .await
            .expect("page reads");

    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].container_id, FIRST_CONTAINER_ID);
    assert_eq!(page.items[0].display_name, "web.1");
    assert_eq!(page.items[0].image_reference, "nginx:1.27");
    assert_eq!(page.items[0].state, ContainerState::Running);
    assert_eq!(page.items[0].status, "Up 3 minutes");
    assert_eq!(
        page.items[0]
            .created_at
            .to_rfc3339_opts(SecondsFormat::Millis, true),
        "2026-09-19T06:00:00.000Z"
    );
    assert_eq!(page.items[1].state, ContainerState::Exited);
    assert_eq!(page.next_cursor.as_deref(), Some(CURSOR));

    let request = &server.requests()[0];
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.target,
        format!(
            "/api/v1/nodes/node-1/containers?limit=2&cursor=v1%3A{}",
            "c".repeat(64)
        )
    );
    let expected = format!("Bearer {ACCESS_CANARY}");
    assert_eq!(request.header("authorization"), Some(expected.as_str()));
    assert!(!request.target.contains(ACCESS_CANARY));

    // The default request asks for the documented first page.
    let page = wait(client.list_containers(
        &access(),
        &node(),
        &ContainerPageRequest::default(),
        &CancellationToken::new(),
    ))
    .await
    .expect("default page reads");
    assert_eq!(page.next_cursor.as_deref(), Some(CURSOR));
    assert_eq!(
        server.requests()[1].target,
        "/api/v1/nodes/node-1/containers?limit=50"
    );
}

#[tokio::test]
async fn list_containers_reads_a_final_page_with_no_cursor() {
    let server = TestServer::plain(vec![respond(200, CONTAINER_PAGE_LAST)]).await;
    let client = plain_client(&server);

    let page = wait(client.list_containers(
        &access(),
        &node(),
        &ContainerPageRequest::default(),
        &CancellationToken::new(),
    ))
    .await
    .expect("final page reads");

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.next_cursor, None);
}

#[tokio::test]
async fn invalid_pagination_makes_no_request() {
    let server = TestServer::plain(vec![respond(200, CONTAINER_PAGE)]).await;
    let client = plain_client(&server);
    let cases = [
        ContainerPageRequest {
            limit: 0,
            cursor: None,
        },
        ContainerPageRequest {
            limit: 101,
            cursor: None,
        },
        ContainerPageRequest {
            limit: 50,
            cursor: Some("bogus".to_owned()),
        },
        ContainerPageRequest {
            limit: 50,
            cursor: Some(format!("v1:{}", "c".repeat(63))),
        },
        ContainerPageRequest {
            limit: 50,
            cursor: Some(format!("v2:{}", "c".repeat(64))),
        },
        ContainerPageRequest {
            limit: 50,
            cursor: Some("c".repeat(68)),
        },
    ];
    for case in cases {
        assert_eq!(
            client
                .list_containers(&access(), &node(), &case, &CancellationToken::new())
                .await
                .err(),
            Some(NativeClientError::InvalidPagination),
            "{case:?} must be rejected locally"
        );
    }
    assert_eq!(server.connection_count(), 0);
    assert_eq!(server.request_count(), 0);
}

#[tokio::test]
async fn foreign_node_provenance_is_rejected() {
    let server = TestServer::plain(vec![respond(200, CONTAINER_PAGE_FOREIGN_NODE)]).await;
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
        Some(NativeClientError::InvalidResponse)
    );
}

#[tokio::test]
async fn invalid_next_cursor_is_rejected() {
    let server = TestServer::plain(vec![respond(200, CONTAINER_PAGE_BAD_CURSOR)]).await;
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
        Some(NativeClientError::InvalidResponse)
    );
}

#[tokio::test]
async fn the_collection_body_limit_is_enforced() {
    let server = TestServer::plain(vec![respond(200, CONTAINER_PAGE)]).await;
    let mut config = NativeClientConfig::new(server.origin());
    config.trust = TrustPolicy::WebPki;
    config.collection_body_limit = 10;
    let client = crate::client::NativeClient::new(config).expect("client builds");

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
        Some(NativeClientError::InvalidResponse),
        "a page above the configured ceiling is rejected"
    );
    assert_eq!(server.request_count(), 1);
}

#[tokio::test]
async fn get_operation_reads_the_bounded_snapshot() {
    let server = TestServer::plain(vec![
        respond(200, OPERATION),
        respond(200, OPERATION_UNCERTAIN),
    ])
    .await;
    let client = plain_client(&server);
    let operation_id = Uuid::parse_str(OPERATION_ID).unwrap();

    let snapshot = wait(client.get_operation(&access(), operation_id, &CancellationToken::new()))
        .await
        .expect("operation reads");

    assert_eq!(snapshot.id, operation_id);
    assert_eq!(snapshot.kind, "service.restart");
    assert_eq!(snapshot.state, OperationState::Running);
    assert!(!snapshot.recoverable);
    assert!(!snapshot.uncertain);
    assert_eq!(snapshot.last_event_sequence, 3);
    assert_eq!(snapshot.last_error_code, None);
    assert_eq!(
        snapshot
            .created_at
            .to_rfc3339_opts(SecondsFormat::Millis, true),
        "2026-09-19T06:50:00.000Z"
    );

    let request = &server.requests()[0];
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, format!("/api/v1/operations/{OPERATION_ID}"));
    let expected = format!("Bearer {ACCESS_CANARY}");
    assert_eq!(request.header("authorization"), Some(expected.as_str()));
    assert!(!request.target.contains(ACCESS_CANARY));

    let snapshot = wait(client.get_operation(&access(), operation_id, &CancellationToken::new()))
        .await
        .expect("uncertain operation reads");
    assert_eq!(snapshot.state, OperationState::Uncertain);
    assert!(snapshot.uncertain);
    assert_eq!(snapshot.last_error_code.as_deref(), Some("engine_timeout"));
}

#[tokio::test]
async fn operation_responses_with_inconsistent_flags_are_rejected() {
    let operation_id = Uuid::parse_str(OPERATION_ID).unwrap();
    for fixture in [OPERATION_INCONSISTENT, OPERATION_UNKNOWN_STATE] {
        let server = TestServer::plain(vec![respond(200, fixture)]).await;
        let client = plain_client(&server);
        assert_eq!(
            client
                .get_operation(&access(), operation_id, &CancellationToken::new())
                .await
                .err(),
            Some(NativeClientError::InvalidResponse)
        );
    }
}

#[tokio::test]
async fn operation_statuses_map_to_closed_variants() {
    let operation_id = Uuid::parse_str(OPERATION_ID).unwrap();
    for (status, expected) in [
        (400, NativeClientError::InvalidRequest),
        (401, NativeClientError::Unauthorized),
        (404, NativeClientError::NotFound),
        (500, NativeClientError::UnexpectedStatus { status: 500 }),
        (503, NativeClientError::ServiceUnavailable),
    ] {
        let server = TestServer::plain(vec![respond(status, r#"{"error":"fixture"}"#)]).await;
        let client = plain_client(&server);
        assert_eq!(
            client
                .get_operation(&access(), operation_id, &CancellationToken::new())
                .await
                .err(),
            Some(expected),
            "status {status}"
        );
        assert_eq!(server.request_count(), 1, "status {status}");
    }
}

#[test]
fn non_canonical_container_ids_are_rejected() {
    let page: serde_json::Value = serde_json::from_str(CONTAINER_PAGE).unwrap();
    for bad_id in [
        "A".repeat(64),
        "a".repeat(63),
        "a".repeat(65),
        format!("{}z", "a".repeat(63)),
        String::new(),
    ] {
        let mut candidate = page.clone();
        candidate["items"] = serde_json::json!([{
            "containerId": bad_id,
            "displayName": "web.1",
            "imageReference": "nginx:1.27",
            "state": "running",
            "status": "Up",
            "createdAt": "2026-09-19T06:00:00.000Z",
            "nodeId": "node-1",
        }]);
        candidate["nextCursor"] = serde_json::Value::Null;
        let decoded: ContainerPageBody = serde_json::from_value(candidate).unwrap();
        assert_eq!(
            decoded.into_page(&node()).err(),
            Some(NativeClientError::InvalidResponse),
            "container id {bad_id:?} must be rejected"
        );
    }
}

#[test]
fn non_canonical_timestamps_are_rejected() {
    for timestamp in [
        "2026-09-19T06:00:00Z",
        "2026-09-19T06:00:00.000+00:00",
        "2026-09-19T06:00:00.000000Z",
        "2026-09-19 06:00:00.000Z",
        "not-a-timestamp",
    ] {
        assert_eq!(
            crate::client::parse_timestamp(timestamp).err(),
            Some(NativeClientError::InvalidResponse),
            "{timestamp} is not the canonical controller rendering"
        );
    }
    assert!(crate::client::parse_timestamp("2026-09-19T06:00:00.000Z").is_ok());
}

#[test]
fn non_canonical_uuids_are_rejected() {
    for value in [
        "018F2E34-0000-7000-8000-0000000000CC",
        "018f2e340000700080000000000000cc",
        "{018f2e34-0000-7000-8000-0000000000cc}",
        "not-a-uuid",
    ] {
        assert_eq!(
            crate::client::parse_uuid(value).err(),
            Some(NativeClientError::InvalidResponse),
            "{value} is not the canonical controller rendering"
        );
    }
    assert!(crate::client::parse_uuid(OPERATION_ID).is_ok());
}

#[tokio::test]
async fn container_and_operation_reads_require_no_anonymous_access() {
    let server = TestServer::plain(vec![respond(401, r#"{"error":"unauthorized"}"#)]).await;
    let client = plain_client(&server);
    // The client presents whatever credential it was given; a rejected one
    // surfaces as Unauthorized for both read methods.
    let access = AccessCredential::parse(ACCESS_CANARY).unwrap();
    assert_eq!(
        client
            .list_containers(
                &access,
                &node(),
                &ContainerPageRequest::default(),
                &CancellationToken::new(),
            )
            .await
            .err(),
        Some(NativeClientError::Unauthorized)
    );
    assert_eq!(
        client
            .get_operation(
                &access,
                Uuid::parse_str(OPERATION_ID).unwrap(),
                &CancellationToken::new(),
            )
            .await
            .err(),
        Some(NativeClientError::Unauthorized)
    );
    assert_eq!(server.request_count(), 2);
    assert!(
        server
            .requests()
            .iter()
            .all(|request| request.header("authorization").is_some())
    );
}

#[test]
fn default_limits_are_the_documented_bounds() {
    assert_eq!(crate::client::DEFAULT_CONNECT_TIMEOUT.as_secs(), 5);
    assert_eq!(crate::client::DEFAULT_REQUEST_DEADLINE.as_secs(), 10);
    assert_eq!(crate::client::DEFAULT_JSON_BODY_LIMIT, 64 * 1024);
    assert_eq!(crate::client::DEFAULT_COLLECTION_BODY_LIMIT, 1024 * 1024);
    assert_eq!(crate::containers::DEFAULT_CONTAINER_LIMIT, 50);
    assert_eq!(crate::containers::MAX_CONTAINER_LIMIT, 100);
    assert_eq!(crate::containers::MAX_CONTAINER_CURSOR_LEN, 67);
}

/// Paths covered by the client's read surface, with their documented method.
const READ_OPENAPI_PATHS: [(&str, &str); 10] = [
    ("/api/v1/nodes", "get"),
    ("/api/v1/nodes/{nodeId}/samples", "get"),
    ("/api/v1/cluster/samples", "get"),
    ("/api/v1/settings", "get"),
    ("/api/v1/settings", "patch"),
    ("/api/v1/nodes/{nodeId}/containers", "get"),
    ("/api/v1/operations/{operation_id}", "get"),
    ("/api/v1/operations/{operation_id}/events", "get"),
    ("/api/v1/operations/{operation_id}/cancel", "post"),
    ("/api/v1/operations/noop", "post"),
];
const READ_OPENAPI_SCHEMAS: [&str; 19] = [
    "NodeListResponse",
    "NodeResponse",
    "NodeStatusResponse",
    "NodeSampleResponse",
    "SampleSeriesResponse",
    "SamplePointResponse",
    "SettingsResponse",
    "SettingsPatchRequest",
    "ContainerErrorResponse",
    "ContainerItemResponse",
    "ContainerPageResponse",
    "ContainerStateResponse",
    "OperationResponse",
    "OperationStateResponse",
    "NodeErrorResponse",
    "OperationEventResponse",
    "OperationEventsResponse",
    "AdmittedResponse",
    "AdmissionErrorResponse",
];
const CONTAINER_STATES: [&str; 9] = [
    "unknown",
    "created",
    "running",
    "paused",
    "restarting",
    "removing",
    "exited",
    "dead",
    "stopping",
];
const OPERATION_STATES: [&str; 6] = [
    "recoverable",
    "running",
    "uncertain",
    "succeeded",
    "failed",
    "cancelled",
];

#[test]
fn openapi_reads_contract_matches_the_checked_artifact() {
    let document = openapi_artifact();
    assert_openapi_snapshot(
        "reads/openapi-contract.json",
        &covered_contract(&document, &READ_OPENAPI_PATHS, &READ_OPENAPI_SCHEMAS),
    );

    assert_eq!(
        response_statuses(&document, "/api/v1/nodes/{nodeId}/containers", "get"),
        BTreeSet::from(["200", "400", "401", "404", "503", "504"].map(str::to_owned))
    );
    let parameters = document["paths"]["/api/v1/nodes/{nodeId}/containers"]["get"]["parameters"]
        .as_array()
        .expect("containers documents parameters");
    let limit = parameters
        .iter()
        .find(|parameter| parameter["name"] == "limit")
        .expect("limit is documented");
    assert_eq!(limit["schema"]["minimum"], 1);
    assert_eq!(limit["schema"]["maximum"], 100);
    let cursor = parameters
        .iter()
        .find(|parameter| parameter["name"] == "cursor")
        .expect("cursor is documented");
    assert_eq!(cursor["schema"]["maxLength"], 67);

    assert_eq!(
        response_statuses(&document, "/api/v1/operations/{operation_id}", "get"),
        BTreeSet::from(["200", "400", "401", "404", "500", "503"].map(str::to_owned))
    );
    assert!(
        document["paths"]["/api/v1/operations/{operation_id}"]["get"]["responses"]["404"]
            ["description"]
            .as_str()
            .is_some_and(|description| description.contains("not visible")),
        "polling documents the anti-oracle 404"
    );

    let page: serde_json::Value =
        serde_json::from_str(CONTAINER_PAGE).expect("page fixture is JSON");
    let item: ContainerItemBody = serde_json::from_value(page["items"][0].clone())
        .expect("item fixture decodes into the wire DTO");
    assert_schema_fields(&document, "ContainerItemResponse", serialized_keys(&item));
    assert_schema_fields(
        &document,
        "ContainerPageResponse",
        round_trip_keys::<ContainerPageBody>(CONTAINER_PAGE),
    );
    assert_schema_fields(
        &document,
        "OperationResponse",
        round_trip_keys::<OperationBody>(OPERATION),
    );
    assert_schema_fields(
        &document,
        "NodeListResponse",
        round_trip_keys::<NodeListBody>(NODES),
    );
    let nodes: serde_json::Value = serde_json::from_str(NODES).unwrap();
    assert_schema_fields(
        &document,
        "NodeResponse",
        nodes["items"][0]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect(),
    );
    assert_schema_fields(
        &document,
        "NodeSampleResponse",
        nodes["items"][0]["latestSample"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect(),
    );
    assert_schema_fields(
        &document,
        "SampleSeriesResponse",
        round_trip_keys::<SampleSeriesBody>(CLUSTER_SAMPLES),
    );
    let series: serde_json::Value = serde_json::from_str(CLUSTER_SAMPLES).unwrap();
    assert_schema_fields(
        &document,
        "SamplePointResponse",
        series["points"][0]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect(),
    );
    assert_eq!(
        document["components"]["schemas"]["NodeStatusResponse"]["enum"],
        json!(["online", "degraded", "offline"])
    );
    for name in ["online", "degraded", "offline"] {
        assert!(serde_json::from_value::<NodeStatus>(json!(name)).is_ok());
    }

    let security = &document["components"]["securitySchemes"]["access_session"];
    assert_eq!(security["type"], "http");
    assert_eq!(security["scheme"], "bearer");
    assert_eq!(security["bearerFormat"], "opaque");
}

#[test]
fn container_and_operation_state_names_are_closed_and_complete() {
    let document = openapi_artifact();
    assert_eq!(
        document["components"]["schemas"]["ContainerStateResponse"]["enum"],
        json!(CONTAINER_STATES)
    );
    assert_eq!(
        document["components"]["schemas"]["OperationStateResponse"]["enum"],
        json!(OPERATION_STATES)
    );

    for name in CONTAINER_STATES {
        assert!(
            serde_json::from_value::<ContainerState>(json!(name)).is_ok(),
            "{name} must decode into the client type"
        );
    }
    for name in OPERATION_STATES {
        assert!(
            serde_json::from_value::<OperationState>(json!(name)).is_ok(),
            "{name} must decode into the client type"
        );
    }
    assert!(
        serde_json::from_value::<ContainerState>(json!("partial")).is_err(),
        "unknown container states are contract drift"
    );
    assert!(
        serde_json::from_value::<OperationState>(json!("partial")).is_err(),
        "unknown operation states are contract drift"
    );
}
