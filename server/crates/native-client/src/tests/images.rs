//! Image list, removal, and pull contracts.

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::support::{
    TestServer, assert_openapi_snapshot, assert_schema_fields, covered_contract, openapi_artifact,
    plain_client, respond, response_statuses, round_trip_keys,
};
use crate::containers::{NodeId, NodeOutcomeStatus};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;
use crate::images::{ImageListBody, ImageNodes};
use crate::operations::IdempotencyKey;

const IMAGE_LIST: &str = include_str!("fixtures/images/image-list.json");
const ACCESS: &str = "kta_11111111111111111111111111111111.2222222222222222222222222222222222222222222222222222222222222222";
const OPERATION_ID: &str = "018f2e34-0000-7000-8000-0000000000cc";
const KEY: &str = "018f2e34-0000-7000-8000-0000000000aa";
const IMAGE_ID: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn access() -> AccessCredential {
    AccessCredential::parse(ACCESS).unwrap()
}

fn key() -> IdempotencyKey {
    IdempotencyKey::parse(KEY).unwrap()
}

fn nodes(names: &[&str]) -> Vec<NodeId> {
    names
        .iter()
        .map(|name| NodeId::parse(name).unwrap())
        .collect()
}

#[tokio::test]
async fn list_images_parses_merged_rows_and_node_outcomes() {
    let server = TestServer::plain(vec![respond(200, IMAGE_LIST)]).await;
    let list = plain_client(&server)
        .list_images(&access(), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(list.items.len(), 2);
    let nginx = &list.items[0];
    assert_eq!(nginx.image_id, IMAGE_ID);
    assert_eq!(nginx.repo_tags, vec!["nginx:1.27", "nginx:latest"]);
    assert_eq!(nginx.used_by, 3);
    assert_eq!(nginx.node_ids, nodes(&["node-1", "worker-1"]));
    assert!(list.items[1].dangling && list.items[1].repo_tags.is_empty());
    assert_eq!(list.nodes[2].status, NodeOutcomeStatus::Unavailable);
    let request = &server.requests()[0];
    assert_eq!(request.target, "/api/v1/images");
    assert!(
        request
            .header("authorization")
            .unwrap()
            .starts_with("Bearer ")
    );
}

#[tokio::test]
async fn image_lists_that_break_the_contract_are_rejected() {
    let valid: serde_json::Value = serde_json::from_str(IMAGE_LIST).unwrap();
    let mut bad_id = valid.clone();
    bad_id["items"][0]["imageId"] = json!("sha256:AAAA");
    let mut foreign = valid.clone();
    foreign["items"][0]["nodeIds"] = json!(["ghost"]);
    let mut nowhere = valid.clone();
    nowhere["items"][0]["nodeIds"] = json!([]);
    let mut extra = valid.clone();
    extra["items"][0]["config"] = json!({"Env": ["secret-canary"]});
    let mut bad_time = valid.clone();
    bad_time["items"][0]["createdAt"] = json!("2026-09-01T10:00:00Z");
    for (label, body) in [
        ("a non-canonical image id", bad_id),
        ("a node outside the membership", foreign),
        ("an image on no node", nowhere),
        ("an unknown field", extra),
        ("a non-canonical time", bad_time),
    ] {
        let server = TestServer::plain(vec![respond(200, &body.to_string())]).await;
        assert_eq!(
            plain_client(&server)
                .list_images(&access(), &CancellationToken::new())
                .await
                .err(),
            Some(NativeClientError::InvalidResponse),
            "{label}"
        );
    }
}

#[tokio::test]
async fn removal_and_pulls_send_the_documented_requests_and_validate_locally() {
    let admitted = json!({"operationId": OPERATION_ID, "replayed": false}).to_string();
    let server = TestServer::plain(vec![
        respond(202, &admitted),
        respond(202, &admitted),
        respond(202, &admitted),
        respond(
            400,
            r#"{"error":"invalid_request","fields":{"reference":"invalid_name"}}"#,
        ),
    ])
    .await;
    let client = plain_client(&server);
    let cancellation = CancellationToken::new();
    client
        .remove_image(
            &access(),
            IMAGE_ID,
            &ImageNodes::All,
            false,
            &key(),
            &cancellation,
        )
        .await
        .unwrap();
    client
        .remove_image(
            &access(),
            IMAGE_ID,
            &ImageNodes::Only(nodes(&["worker-1"])),
            true,
            &key(),
            &cancellation,
        )
        .await
        .unwrap();
    client
        .pull_image(
            &access(),
            "alpine:3.20",
            &nodes(&["node-1", "worker-1"]),
            &key(),
            &cancellation,
        )
        .await
        .unwrap();
    assert_eq!(
        client
            .pull_image(
                &access(),
                "Alpine",
                &nodes(&["node-1"]),
                &key(),
                &cancellation
            )
            .await
            .err(),
        Some(NativeClientError::InvalidRequest),
        "the controller's grammar check is a plain rejection"
    );
    let requests = server.requests();
    assert_eq!(requests[0].method, "DELETE");
    assert_eq!(requests[0].target, format!("/api/v1/images/{IMAGE_ID}"));
    assert_eq!(requests[0].header("idempotency-key"), Some(KEY));
    assert_eq!(requests[0].body, r#"{"nodeIds":"all"}"#);
    assert_eq!(requests[1].body, r#"{"nodeIds":["worker-1"],"force":true}"#);
    assert_eq!(requests[2].method, "POST");
    assert_eq!(requests[2].target, "/api/v1/images/pulls");
    assert_eq!(
        requests[2].body,
        r#"{"reference":"alpine:3.20","nodeIds":["node-1","worker-1"]}"#
    );

    // Nothing leaves the client for these.
    let duplicate = nodes(&["node-1", "node-1"]);
    for (image_id, targets) in [
        ("sha256:short", ImageNodes::All),
        (IMAGE_ID, ImageNodes::Only(Vec::new())),
        (IMAGE_ID, ImageNodes::Only(duplicate.clone())),
    ] {
        assert_eq!(
            client
                .remove_image(&access(), image_id, &targets, false, &key(), &cancellation)
                .await
                .err(),
            Some(NativeClientError::InvalidRequest)
        );
    }
    let overlong = "a".repeat(513);
    for (reference, targets) in [
        ("", nodes(&["node-1"])),
        (overlong.as_str(), nodes(&["node-1"])),
        ("has space", nodes(&["node-1"])),
        ("alpine", Vec::new()),
        ("alpine", duplicate),
    ] {
        assert_eq!(
            client
                .pull_image(&access(), reference, &targets, &key(), &cancellation)
                .await
                .err(),
            Some(NativeClientError::InvalidRequest),
            "{reference:?}"
        );
    }
    assert_eq!(server.request_count(), 4);
}

const IMAGE_OPENAPI_PATHS: [(&str, &str); 3] = [
    ("/api/v1/images", "get"),
    ("/api/v1/images/{imageId}", "delete"),
    ("/api/v1/images/pulls", "post"),
];
const IMAGE_OPENAPI_SCHEMAS: [&str; 5] = [
    "ImageListResponse",
    "ImageResponse",
    "ImageFieldErrorResponse",
    "RemoveImageRequest",
    "PullImageRequest",
];

#[test]
fn openapi_images_contract_matches_the_checked_artifact() {
    let document = openapi_artifact();
    assert_openapi_snapshot(
        "images/openapi-contract.json",
        &covered_contract(&document, &IMAGE_OPENAPI_PATHS, &IMAGE_OPENAPI_SCHEMAS),
    );
    for (path, method) in [
        ("/api/v1/images/{imageId}", "delete"),
        ("/api/v1/images/pulls", "post"),
    ] {
        let statuses = response_statuses(&document, path, method);
        assert!(
            statuses.contains("202") && statuses.contains("400"),
            "{path}"
        );
    }
    assert_schema_fields(
        &document,
        "ImageListResponse",
        round_trip_keys::<ImageListBody>(IMAGE_LIST),
    );
    let list: serde_json::Value = serde_json::from_str(IMAGE_LIST).unwrap();
    assert_schema_fields(
        &document,
        "ImageResponse",
        list["items"][0]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect(),
    );
}
