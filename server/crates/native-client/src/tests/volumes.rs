//! Volume list, create, and remove contracts.

use std::collections::BTreeMap;

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::support::{
    TestServer, assert_openapi_snapshot, assert_schema_fields, covered_contract, openapi_artifact,
    plain_client, respond, response_statuses, round_trip_keys,
};
use crate::containers::NodeId;
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;
use crate::operations::IdempotencyKey;
use crate::volumes::{VolumeListBody, VolumeName, VolumeQuery, VolumeSpec};

const VOLUME_LIST: &str = include_str!("fixtures/volumes/volume-list.json");
const ACCESS: &str = "kta_11111111111111111111111111111111.2222222222222222222222222222222222222222222222222222222222222222";
const OPERATION_ID: &str = "018f2e34-0000-7000-8000-0000000000cc";
const KEY: &str = "018f2e34-0000-7000-8000-0000000000aa";

fn access() -> AccessCredential {
    AccessCredential::parse(ACCESS).unwrap()
}

fn key() -> IdempotencyKey {
    IdempotencyKey::parse(KEY).unwrap()
}

fn node(id: &str) -> NodeId {
    NodeId::parse(id).unwrap()
}

#[tokio::test]
async fn list_volumes_parses_rows_per_node_and_sends_the_conflict_query() {
    let server =
        TestServer::plain(vec![respond(200, VOLUME_LIST), respond(200, VOLUME_LIST)]).await;
    let client = plain_client(&server);
    let list = client
        .list_volumes(
            &access(),
            &VolumeQuery::default(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(list.items.len(), 2);
    let redis = &list.items[0];
    assert_eq!(redis.node_id, node("node-1"));
    assert_eq!(redis.users, vec!["toolbox_redis.1.x7kq"]);
    assert_eq!(redis.size_bytes, Some(4096));
    assert!(redis.created_at.is_some());
    assert_eq!(list.items[1].size_bytes, None);
    assert_eq!(list.items[1].created_at, None);
    client
        .list_volumes(
            &access(),
            &VolumeQuery {
                node: Some(node("worker-1")),
                name: Some(VolumeName::parse("pg-data").unwrap()),
            },
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    let targets: Vec<_> = server
        .requests()
        .into_iter()
        .map(|request| request.target)
        .collect();
    assert_eq!(
        targets,
        vec![
            "/api/v1/volumes",
            "/api/v1/volumes?node=worker-1&name=pg-data"
        ]
    );
}

#[tokio::test]
async fn volume_lists_that_break_the_contract_are_rejected() {
    let valid: serde_json::Value = serde_json::from_str(VOLUME_LIST).unwrap();
    let mut foreign = valid.clone();
    foreign["items"][0]["nodeId"] = json!("ghost");
    let mut bad_name = valid.clone();
    bad_name["items"][0]["name"] = json!("bad\nname");
    let mut extra = valid.clone();
    extra["items"][0]["options"] = json!({"device": "secret-canary"});
    let mut many_users = valid.clone();
    many_users["items"][0]["users"] = json!(vec!["x"; 17]);
    for (label, body) in [
        ("a node outside the membership", foreign),
        ("an invalid name", bad_name),
        ("an unknown field", extra),
        ("too many users", many_users),
    ] {
        let server = TestServer::plain(vec![respond(200, &body.to_string())]).await;
        assert_eq!(
            plain_client(&server)
                .list_volumes(
                    &access(),
                    &VolumeQuery::default(),
                    &CancellationToken::new()
                )
                .await
                .err(),
            Some(NativeClientError::InvalidResponse),
            "{label}"
        );
    }

    // A name Kontainer would never create still lists.
    let mut odd_name = valid;
    odd_name["items"][0]["name"] = json!("-legacy data");
    let server = TestServer::plain(vec![respond(200, &odd_name.to_string())]).await;
    let list = plain_client(&server)
        .list_volumes(
            &access(),
            &VolumeQuery::default(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(list.items[0].name, "-legacy data");
}

#[tokio::test]
async fn create_and_remove_send_the_documented_requests_and_validate_locally() {
    let admitted = json!({"operationId": OPERATION_ID, "replayed": false}).to_string();
    let server = TestServer::plain(vec![respond(202, &admitted), respond(202, &admitted)]).await;
    let client = plain_client(&server);
    let cancellation = CancellationToken::new();
    let spec = VolumeSpec {
        node_id: node("worker-1"),
        name: VolumeName::parse("pg-data").unwrap(),
        labels: BTreeMap::from([("app".to_owned(), "billing".to_owned())]),
    };
    client
        .create_volume(&access(), &spec, &key(), &cancellation)
        .await
        .unwrap();
    client
        .remove_volume(
            &access(),
            &node("worker-1"),
            &VolumeName::parse("pg-data").unwrap(),
            &key(),
            &cancellation,
        )
        .await
        .unwrap();
    let requests = server.requests();
    assert_eq!(
        (requests[0].method.as_str(), requests[0].target.as_str()),
        ("POST", "/api/v1/volumes")
    );
    assert_eq!(
        requests[0].body,
        r#"{"nodeId":"worker-1","name":"pg-data","driver":"local","labels":{"app":"billing"}}"#
    );
    assert_eq!(requests[1].method, "DELETE");
    assert_eq!(requests[1].target, "/api/v1/volumes/worker-1/pg-data");
    assert_eq!(requests[1].header("idempotency-key"), Some(KEY));

    for name in ["", "a", "-data", "has space", "sl/ash", &"a".repeat(256)] {
        assert_eq!(
            VolumeName::parse(name).err(),
            Some(NativeClientError::InvalidRequest),
            "{name:?}"
        );
    }
    let bad_labels = VolumeSpec {
        labels: BTreeMap::from([("a=b".to_owned(), String::new())]),
        ..spec
    };
    assert_eq!(
        client
            .create_volume(&access(), &bad_labels, &key(), &cancellation)
            .await
            .err(),
        Some(NativeClientError::InvalidRequest)
    );
    assert_eq!(server.request_count(), 2);
}

const VOLUME_OPENAPI_PATHS: [(&str, &str); 3] = [
    ("/api/v1/volumes", "get"),
    ("/api/v1/volumes", "post"),
    ("/api/v1/volumes/{nodeId}/{name}", "delete"),
];
const VOLUME_OPENAPI_SCHEMAS: [&str; 4] = [
    "VolumeListResponse",
    "VolumeResponse",
    "CreateVolumeRequest",
    "FieldErrorResponse",
];

#[test]
fn openapi_volumes_contract_matches_the_checked_artifact() {
    let document = openapi_artifact();
    assert_openapi_snapshot(
        "volumes/openapi-contract.json",
        &covered_contract(&document, &VOLUME_OPENAPI_PATHS, &VOLUME_OPENAPI_SCHEMAS),
    );
    for (path, method) in [
        ("/api/v1/volumes", "post"),
        ("/api/v1/volumes/{nodeId}/{name}", "delete"),
    ] {
        let statuses = response_statuses(&document, path, method);
        assert!(
            statuses.contains("202") && statuses.contains("400"),
            "{path}"
        );
    }
    assert!(response_statuses(&document, "/api/v1/volumes", "get").contains("404"));
    assert_schema_fields(
        &document,
        "VolumeListResponse",
        round_trip_keys::<VolumeListBody>(VOLUME_LIST),
    );
    let list: serde_json::Value = serde_json::from_str(VOLUME_LIST).unwrap();
    assert_schema_fields(
        &document,
        "VolumeResponse",
        list["items"][0]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect(),
    );
}
