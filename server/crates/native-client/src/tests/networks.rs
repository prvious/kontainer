//! Network list, inspect, create, and remove contracts.

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::support::{
    TestServer, assert_openapi_snapshot, assert_schema_fields, covered_contract, openapi_artifact,
    plain_client, respond, response_statuses, round_trip_keys,
};
use crate::containers::NodeId;
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;
use crate::networks::{
    ManagerStatus, NetworkId, NetworkListBody, NetworkPlacement, NetworkScope, NetworkSpec,
};
use crate::operations::IdempotencyKey;

const NETWORK_LIST: &str = include_str!("fixtures/networks/network-list.json");
const ACCESS: &str = "kta_11111111111111111111111111111111.2222222222222222222222222222222222222222222222222222222222222222";
const OPERATION_ID: &str = "018f2e34-0000-7000-8000-0000000000cc";
const KEY: &str = "018f2e34-0000-7000-8000-0000000000aa";
const SWARM_ID: &str = "x7kq2m9p0r1s3t5u7w9y0z2b4";

fn access() -> AccessCredential {
    AccessCredential::parse(ACCESS).unwrap()
}

fn key() -> IdempotencyKey {
    IdempotencyKey::parse(KEY).unwrap()
}

#[tokio::test]
async fn list_and_inspect_networks_parse_scope_nodes_and_attached_containers() {
    let attached = json!({"containers": [{"nodeId": "worker-1", "containerId": "c".repeat(64), "name": "toolbox_redis.1"}]}).to_string();
    let server = TestServer::plain(vec![
        respond(200, NETWORK_LIST),
        respond(200, &attached),
        respond(200, &attached),
    ])
    .await;
    let client = plain_client(&server);
    let list = client
        .list_networks(&access(), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(list.manager, ManagerStatus::Ok);
    assert_eq!(list.items[0].scope, NetworkScope::Local);
    assert!(list.items[0].builtin);
    let swarm = &list.items[1];
    assert_eq!(
        (swarm.scope, swarm.containers, swarm.node_ids.len()),
        (NetworkScope::Swarm, 5, 2)
    );
    let id = NetworkId::parse(SWARM_ID).unwrap();
    let containers = client
        .inspect_network(&access(), &id, None, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(containers[0].name, "toolbox_redis.1");
    client
        .inspect_network(
            &access(),
            &id,
            Some(&NodeId::parse("worker-1").unwrap()),
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
            "/api/v1/networks".to_owned(),
            format!("/api/v1/networks/{SWARM_ID}"),
            format!("/api/v1/networks/{SWARM_ID}?node=worker-1"),
        ]
    );
}

#[tokio::test]
async fn network_lists_that_break_the_contract_are_rejected() {
    let valid: serde_json::Value = serde_json::from_str(NETWORK_LIST).unwrap();
    let mut two_nodes_local = valid.clone();
    two_nodes_local["items"][0]["nodeIds"] = json!(["node-1", "worker-1"]);
    let mut foreign = valid.clone();
    foreign["items"][1]["nodeIds"] = json!(["ghost"]);
    let mut bad_scope = valid.clone();
    bad_scope["items"][1]["scope"] = json!("global");
    let mut extra = valid.clone();
    extra["items"][0]["options"] = json!({"secret": "canary"});
    for (label, body) in [
        ("a local network on two nodes", two_nodes_local),
        ("a node outside the membership", foreign),
        ("an unknown scope", bad_scope),
        ("an unknown field", extra),
    ] {
        let server = TestServer::plain(vec![respond(200, &body.to_string())]).await;
        assert_eq!(
            plain_client(&server)
                .list_networks(&access(), &CancellationToken::new())
                .await
                .err(),
            Some(NativeClientError::InvalidResponse),
            "{label}"
        );
    }
}

#[tokio::test]
async fn create_and_remove_send_the_documented_requests() {
    let admitted = json!({"operationId": OPERATION_ID, "replayed": false}).to_string();
    let server = TestServer::plain(vec![
        respond(202, &admitted),
        respond(202, &admitted),
        respond(202, &admitted),
        respond(202, &admitted),
    ])
    .await;
    let client = plain_client(&server);
    let cancellation = CancellationToken::new();
    let overlay = NetworkSpec {
        name: "app-network".to_owned(),
        placement: NetworkPlacement::Overlay { attachable: true },
        subnet: Some("10.0.9.0/24".to_owned()),
    };
    client
        .create_network(&access(), &overlay, &key(), &cancellation)
        .await
        .unwrap();
    let bridge = NetworkSpec {
        name: "local-net".to_owned(),
        placement: NetworkPlacement::Bridge {
            node_id: NodeId::parse("worker-2").unwrap(),
        },
        subnet: None,
    };
    client
        .create_network(&access(), &bridge, &key(), &cancellation)
        .await
        .unwrap();
    let id = NetworkId::parse(SWARM_ID).unwrap();
    client
        .remove_network(&access(), &id, None, &key(), &cancellation)
        .await
        .unwrap();
    let local = NetworkId::parse(&"b".repeat(64)).unwrap();
    client
        .remove_network(
            &access(),
            &local,
            Some(&NodeId::parse("worker-2").unwrap()),
            &key(),
            &cancellation,
        )
        .await
        .unwrap();
    let requests = server.requests();
    assert_eq!(
        requests[0].body,
        r#"{"name":"app-network","driver":"overlay","attachable":true,"subnet":"10.0.9.0/24"}"#
    );
    assert_eq!(
        requests[1].body,
        r#"{"name":"local-net","driver":"bridge","attachable":false,"nodeId":"worker-2"}"#
    );
    assert_eq!(
        (requests[2].method.as_str(), requests[2].target.clone()),
        ("DELETE", format!("/api/v1/networks/{SWARM_ID}"))
    );
    assert_eq!(
        requests[3].target,
        format!("/api/v1/networks/{}?node=worker-2", "b".repeat(64))
    );

    for bad in ["short", &"A".repeat(64), &"a".repeat(63)] {
        assert_eq!(
            NetworkId::parse(bad).err(),
            Some(NativeClientError::InvalidRequest),
            "{bad}"
        );
    }
    let invalid = NetworkSpec {
        name: "-bad".to_owned(),
        ..overlay
    };
    assert_eq!(
        client
            .create_network(&access(), &invalid, &key(), &cancellation)
            .await
            .err(),
        Some(NativeClientError::InvalidRequest)
    );
    assert_eq!(server.request_count(), 4);
}

const NETWORK_OPENAPI_PATHS: [(&str, &str); 4] = [
    ("/api/v1/networks", "get"),
    ("/api/v1/networks", "post"),
    ("/api/v1/networks/{networkId}", "get"),
    ("/api/v1/networks/{networkId}", "delete"),
];
const NETWORK_OPENAPI_SCHEMAS: [&str; 5] = [
    "NetworkListResponse",
    "NetworkResponse",
    "NetworkContainersResponse",
    "AttachedContainerResponse",
    "CreateNetworkRequest",
];

#[test]
fn openapi_networks_contract_matches_the_checked_artifact() {
    let document = openapi_artifact();
    assert_openapi_snapshot(
        "networks/openapi-contract.json",
        &covered_contract(&document, &NETWORK_OPENAPI_PATHS, &NETWORK_OPENAPI_SCHEMAS),
    );
    let statuses = response_statuses(&document, "/api/v1/networks/{networkId}", "delete");
    assert!(statuses.contains("202") && statuses.contains("400") && statuses.contains("503"));
    assert_schema_fields(
        &document,
        "NetworkListResponse",
        round_trip_keys::<NetworkListBody>(NETWORK_LIST),
    );
    let list: serde_json::Value = serde_json::from_str(NETWORK_LIST).unwrap();
    assert_schema_fields(
        &document,
        "NetworkResponse",
        list["items"][0]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect(),
    );
}
