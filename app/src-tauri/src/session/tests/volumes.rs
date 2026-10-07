//! Volume IPC contracts: the list view, the conflict query, create, and remove under refresh.

use std::collections::BTreeMap;

use kontainer_native_client::{
    AdmittedOperation, NativeClientError, NodeId, NodeOutcome, NodeOutcomeStatus, VolumeItem,
    VolumeList,
};

use crate::session::manager::{CreateVolumeInput, InventoryError, VolumeListView};

use super::fakes::{future_ms, issued, Harness};

const FIXTURE: &str = include_str!("../../../../tests/fixtures/volumes-ipc.json");
const LIB_SOURCE: &str = include_str!("../../lib.rs");
const KEY: &str = "018f2e34-0000-7000-8000-0000000000aa";

fn volume_list() -> VolumeList {
    let outcome = |node: &str| NodeOutcome {
        node_id: NodeId::parse(node).unwrap(),
        status: NodeOutcomeStatus::Ok,
        message: None,
    };
    VolumeList {
        items: vec![
            VolumeItem {
                node_id: NodeId::parse("manager").unwrap(),
                name: "toolbox_redis-data".to_owned(),
                driver: "local".to_owned(),
                mount_point: "/var/lib/docker/volumes/toolbox_redis-data/_data".to_owned(),
                created_at: Some("2026-09-21T14:13:20.000Z".parse().unwrap()),
                labels: BTreeMap::from([(
                    "com.docker.stack.namespace".to_owned(),
                    "toolbox".to_owned(),
                )]),
                used_by: 1,
                users: vec!["toolbox_redis.1.x7kq".to_owned()],
                size_bytes: Some(536_870_912),
            },
            VolumeItem {
                node_id: NodeId::parse("worker-1").unwrap(),
                name: "scratch".to_owned(),
                driver: "local".to_owned(),
                mount_point: "/var/lib/docker/volumes/scratch/_data".to_owned(),
                created_at: Some("2026-09-27T08:00:00.000Z".parse().unwrap()),
                labels: BTreeMap::new(),
                used_by: 0,
                users: Vec::new(),
                size_bytes: None,
            },
        ],
        nodes: vec![outcome("manager"), outcome("worker-1"), outcome("worker-2")],
        snapshot_at: "2026-09-28T12:05:00.000Z".parse().unwrap(),
    }
}

fn admitted(id: u128) -> Result<AdmittedOperation, NativeClientError> {
    Ok(AdmittedOperation {
        operation_id: uuid::Uuid::from_u128(id),
        replayed: false,
    })
}

#[test]
fn volume_views_and_commands_match_the_shared_fixture() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        serde_json::to_value(VolumeListView::from(volume_list())).unwrap(),
        fixture["volumeList"]
    );
    for command in fixture["commands"].as_array().unwrap() {
        let domain = command[0].as_str().unwrap();
        let name = command[1].as_str().unwrap();
        assert!(
            LIB_SOURCE.contains(&format!("actions::{domain}::{name}::{name}")),
            "{name} is registered"
        );
    }
    assert!(
        serde_json::from_value::<CreateVolumeInput>(
            serde_json::json!({"nodeId": "manager", "name": "pg", "driver": "nfs"})
        )
        .is_err(),
        "the driver is not React's to choose"
    );
}

#[tokio::test]
async fn volume_reads_and_mutations_carry_their_targets_and_retry_once_on_401() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    {
        let mut script = harness.controllers.script();
        script.volumes.push_back(Ok(volume_list()));
        script.volumes.push_back(Ok(volume_list()));
        script
            .admissions
            .push_back(Err(NativeClientError::Unauthorized));
        script
            .refresh
            .push_back(Ok(issued(2, future_ms(), future_ms())));
        script.admissions.push_back(admitted(0xcc));
        script.admissions.push_back(admitted(0xdd));
    }
    let list = harness
        .manager
        .volumes(view.profile_id, None, None)
        .await
        .unwrap();
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(serde_json::to_value(list).unwrap(), fixture["volumeList"]);
    harness
        .manager
        .volumes(view.profile_id, Some("worker-1"), Some("pg-data"))
        .await
        .unwrap();
    assert!(harness
        .entries()
        .iter()
        .any(|entry| entry.starts_with("volumes:") && entry.ends_with(":worker-1:pg-data")));

    let input = CreateVolumeInput {
        node_id: "worker-1".to_owned(),
        name: "pg-data".to_owned(),
        labels: BTreeMap::from([("app".to_owned(), "billing".to_owned())]),
    };
    let created = harness
        .manager
        .create_volume(view.profile_id, &input, KEY)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(created).unwrap()["operationId"],
        uuid::Uuid::from_u128(0xcc).to_string()
    );
    let creates: Vec<_> = harness
        .entries()
        .into_iter()
        .filter(|entry| entry.starts_with("create_volume:"))
        .collect();
    assert_eq!(creates.len(), 2, "one 401, one retry with the same key");
    assert!(creates
        .iter()
        .all(|entry| entry.ends_with(&format!(":worker-1:pg-data:app=billing:{KEY}"))));

    harness
        .manager
        .remove_volume(view.profile_id, "manager", "toolbox_redis-data", KEY)
        .await
        .unwrap();
    assert!(harness
        .entries()
        .iter()
        .any(|entry| entry.starts_with("remove_volume:")
            && entry.ends_with(&format!(":manager:toolbox_redis-data:{KEY}"))));

    // Invalid input never leaves the app.
    assert_eq!(
        harness
            .manager
            .volumes(view.profile_id, None, Some("a"))
            .await
            .err(),
        Some(InventoryError::InvalidRequest)
    );
    let bad = CreateVolumeInput {
        name: "-bad".to_owned(),
        ..input
    };
    assert!(harness
        .manager
        .create_volume(view.profile_id, &bad, KEY)
        .await
        .is_err());
    assert_eq!(
        harness
            .manager
            .remove_volume(view.profile_id, "../x", "scratch", KEY)
            .await
            .err(),
        Some(InventoryError::InvalidNodeId)
    );
    assert_eq!(harness.count("create_volume:"), 2);
    assert_eq!(harness.count("remove_volume:"), 1);
    assert_eq!(harness.count("volumes:"), 2);
}
