//! Network IPC contracts: the list view, attached containers, create, and remove under refresh.

use kontainer_native_client::{
    AdmittedOperation, AttachedContainer, ManagerStatus, NativeClientError, NetworkItem,
    NetworkList, NetworkScope, NodeId, NodeOutcome, NodeOutcomeStatus,
};

use crate::session::manager::{CreateNetworkInput, InventoryError, NetworkListView};

use super::fakes::{future_ms, issued, Harness};

const FIXTURE: &str = include_str!("../../../../tests/fixtures/networks-ipc.json");
const LIB_SOURCE: &str = include_str!("../../lib.rs");
const KEY: &str = "018f2e34-0000-7000-8000-0000000000aa";
const SWARM_ID: &str = "x7kq2m9p0r1s3t5u7w9y0z2b4";

fn node(id: &str) -> NodeId {
    NodeId::parse(id).unwrap()
}

fn network_list() -> NetworkList {
    let item = |id: &str, name: &str, scope, containers, created: Option<&str>, nodes: &[&str]| {
        NetworkItem {
            id: id.to_owned(),
            name: name.to_owned(),
            driver: if scope == NetworkScope::Swarm {
                "overlay"
            } else {
                "bridge"
            }
            .to_owned(),
            scope,
            subnets: match name {
                "bridge" => vec!["172.17.0.0/16".to_owned()],
                "toolbox_kontainer" => vec!["10.0.1.0/24".to_owned()],
                _ => Vec::new(),
            },
            containers,
            created_at: created.map(|at| at.parse().unwrap()),
            attachable: name == "toolbox_kontainer",
            internal: false,
            builtin: name == "bridge",
            node_ids: nodes.iter().map(|id| node(id)).collect(),
        }
    };
    let outcome = |id: &str| NodeOutcome {
        node_id: node(id),
        status: NodeOutcomeStatus::Ok,
        message: None,
    };
    NetworkList {
        items: vec![
            item(
                &"b".repeat(64),
                "bridge",
                NetworkScope::Local,
                0,
                Some("2026-09-20T10:00:00.000Z"),
                &["manager"],
            ),
            item(
                SWARM_ID,
                "toolbox_kontainer",
                NetworkScope::Swarm,
                6,
                Some("2026-09-21T09:00:00.000Z"),
                &["manager", "worker-1"],
            ),
            item(
                "e1e1e1e1e1e1e1e1e1e1e1e1e",
                "empty-overlay",
                NetworkScope::Swarm,
                0,
                None,
                &[],
            ),
        ],
        nodes: vec![outcome("manager"), outcome("worker-1")],
        manager: ManagerStatus::Ok,
        snapshot_at: "2026-09-28T12:05:00.000Z".parse().unwrap(),
    }
}

fn attached() -> Vec<AttachedContainer> {
    vec![
        AttachedContainer {
            node_id: node("manager"),
            container_id: "c".repeat(64),
            name: "toolbox_nginx.1.x7kq".to_owned(),
        },
        AttachedContainer {
            node_id: node("worker-1"),
            container_id: "d".repeat(64),
            name: "toolbox_redis.1.a2b3".to_owned(),
        },
    ]
}

fn admitted(id: u128) -> Result<AdmittedOperation, NativeClientError> {
    Ok(AdmittedOperation {
        operation_id: uuid::Uuid::from_u128(id),
        replayed: false,
    })
}

#[test]
fn network_views_and_commands_match_the_shared_fixture() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        serde_json::to_value(NetworkListView::from(network_list())).unwrap(),
        fixture["networkList"]
    );
    for command in fixture["commands"].as_array().unwrap() {
        let domain = command[0].as_str().unwrap();
        let name = command[1].as_str().unwrap();
        assert!(
            LIB_SOURCE.contains(&format!("actions::{domain}::{name}::{name}")),
            "{name} is registered"
        );
    }
}

#[tokio::test]
async fn network_calls_carry_their_targets_and_retry_once_on_401() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    {
        let mut script = harness.controllers.script();
        script.networks.push_back(Ok(network_list()));
        script.attached.push_back(Ok(attached()));
        script
            .admissions
            .push_back(Err(NativeClientError::Unauthorized));
        script
            .refresh
            .push_back(Ok(issued(2, future_ms(), future_ms())));
        script.admissions.push_back(admitted(0xcc));
        script.admissions.push_back(admitted(0xdd));
        script.admissions.push_back(admitted(0xee));
    }
    let list = harness.manager.networks(view.profile_id).await.unwrap();
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(serde_json::to_value(list).unwrap(), fixture["networkList"]);
    let containers = harness
        .manager
        .network_containers(view.profile_id, SWARM_ID, None)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(containers).unwrap(),
        fixture["attached"]
    );
    assert!(harness
        .entries()
        .iter()
        .any(|entry| entry.ends_with(&format!(":{SWARM_ID}:swarm"))));

    let overlay = CreateNetworkInput {
        name: "app-network".to_owned(),
        driver: "overlay".to_owned(),
        attachable: true,
        node_id: None,
        subnet: Some("10.0.9.0/24".to_owned()),
    };
    harness
        .manager
        .create_network(view.profile_id, &overlay, KEY)
        .await
        .unwrap();
    let creates: Vec<_> = harness
        .entries()
        .into_iter()
        .filter(|entry| entry.starts_with("create_network:"))
        .collect();
    assert_eq!(creates.len(), 2, "one 401, one retry with the same key");
    assert!(creates
        .iter()
        .all(|entry| entry.ends_with(&format!(":app-network:overlay:true:10.0.9.0/24:{KEY}"))));
    let bridge = CreateNetworkInput {
        name: "local-net".to_owned(),
        driver: "bridge".to_owned(),
        attachable: false,
        node_id: Some("worker-1".to_owned()),
        subnet: Some(String::new()),
    };
    harness
        .manager
        .create_network(view.profile_id, &bridge, KEY)
        .await
        .unwrap();
    assert!(harness
        .entries()
        .iter()
        .any(|entry| entry.ends_with(&format!(":local-net:bridge:worker-1:none:{KEY}"))));
    harness
        .manager
        .remove_network(view.profile_id, &"b".repeat(64), Some("manager"), KEY)
        .await
        .unwrap();
    assert!(harness
        .entries()
        .iter()
        .any(|entry| entry.starts_with("remove_network:")
            && entry.ends_with(&format!(":{}:manager:{KEY}", "b".repeat(64)))));

    // Invalid input never leaves the app.
    for input in [
        CreateNetworkInput {
            driver: "bridge".to_owned(),
            node_id: None,
            ..overlay.clone()
        },
        CreateNetworkInput {
            driver: "overlay".to_owned(),
            node_id: Some("manager".to_owned()),
            ..overlay.clone()
        },
        CreateNetworkInput {
            driver: "macvlan".to_owned(),
            ..overlay.clone()
        },
    ] {
        assert_eq!(
            harness
                .manager
                .create_network(view.profile_id, &input, KEY)
                .await
                .err(),
            Some(InventoryError::InvalidRequest)
        );
    }
    assert!(harness
        .manager
        .remove_network(view.profile_id, "short", None, KEY)
        .await
        .is_err());
    assert_eq!(harness.count("create_network:"), 3);
    assert_eq!(harness.count("remove_network:"), 1);
}
