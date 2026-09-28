//! Inventory IPC contract and custody behavior under read, refresh, and cancellation.

use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use kontainer_native_client::{
    ContainerItem, ContainerPage, ContainerState, NativeClientError, NodeId,
};
use tokio::sync::Notify;

use crate::session::{errors::SessionError, manager::InventoryError, models::AnchorState};

use super::fakes::{future_ms, issued, Harness, PROFILE_ID};

const FIXTURE: &str = include_str!("../../../../tests/fixtures/inventory-ipc.json");
const LIB_SOURCE: &str = include_str!("../../lib.rs");

fn page() -> ContainerPage {
    ContainerPage {
        items: vec![ContainerItem {
            container_id: "a".repeat(64),
            display_name: "web".to_owned(),
            image_reference: "nginx:latest".to_owned(),
            state: ContainerState::Running,
            status: "Up 3 minutes".to_owned(),
            created_at: "2026-09-19T06:00:00.000Z".parse().unwrap(),
            node_id: "node-1".to_owned(),
        }],
        next_cursor: Some(format!("v1:{}", "a".repeat(64))),
    }
}

#[test]
fn inventory_ipc_fields_codes_and_commands_match_shared_fixture() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        serde_json::to_value(super::super::manager::ContainerPageView::from(page())).unwrap(),
        fixture["containerPage"]
    );
    assert_eq!(fixture["configuredNode"], "node-1");
    let codes: Vec<_> = InventoryError::ALL
        .iter()
        .map(|code| serde_json::to_value(code).unwrap())
        .collect();
    assert_eq!(serde_json::to_value(codes).unwrap(), fixture["errorCodes"]);
    for command in fixture["commands"].as_array().unwrap() {
        let name = command.as_str().unwrap();
        assert!(LIB_SOURCE.contains(&format!("actions::inventory::{name}::{name}")));
    }
    assert!(!LIB_SOURCE.contains("actions::agent::list_tasks::list_tasks"));
}

#[test]
fn native_failures_map_to_specific_closed_ipc_codes() {
    for (native, expected) in [
        (
            NativeClientError::InvalidPagination,
            InventoryError::InvalidPagination,
        ),
        (
            NativeClientError::InvalidNodeId,
            InventoryError::InvalidNodeId,
        ),
        (
            NativeClientError::Unauthorized,
            InventoryError::ReloginRequired,
        ),
        (NativeClientError::NotFound, InventoryError::NodeNotFound),
        (
            NativeClientError::AgentUnavailable,
            InventoryError::AgentUnavailable,
        ),
        (
            NativeClientError::AgentTimeout,
            InventoryError::AgentTimeout,
        ),
        (
            NativeClientError::ServiceUnavailable,
            InventoryError::ControllerUnavailable,
        ),
        (
            NativeClientError::GatewayTimeout,
            InventoryError::ControllerUnavailable,
        ),
    ] {
        assert_eq!(InventoryError::from(native), expected);
    }
}

#[tokio::test]
async fn inventory_reads_use_only_the_active_profile_and_serialize_the_page() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    harness
        .controllers
        .script()
        .discover
        .push_back(Ok(Some(NodeId::parse("node-1").unwrap())));
    harness
        .controllers
        .script()
        .containers
        .push_back(Ok(page()));

    assert_eq!(
        harness
            .manager
            .configured_node(view.profile_id)
            .await
            .unwrap(),
        Some("node-1".to_owned())
    );
    let result = harness
        .manager
        .container_inventory(view.profile_id, "node-1", None, 50)
        .await
        .unwrap();
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        fixture["containerPage"]
    );
    assert_eq!(harness.count("discover:"), 1);
    assert_eq!(harness.count("containers:"), 1);
    assert_eq!(harness.count("refresh:"), 0);
    assert_eq!(
        harness.manager.configured_node(PROFILE_ID).await.err(),
        Some(InventoryError::Unauthenticated)
    );
    assert_eq!(
        harness
            .manager
            .container_inventory(view.profile_id, "bad/node", None, 50)
            .await
            .err(),
        Some(InventoryError::InvalidNodeId)
    );
    assert_eq!(harness.count("discover:"), 1);
    assert_eq!(harness.count("containers:"), 1);
}

#[tokio::test]
async fn one_401_rotates_in_order_then_retries_the_same_read_once() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    harness
        .controllers
        .script()
        .discover
        .push_back(Err(NativeClientError::Unauthorized));
    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));
    harness
        .controllers
        .script()
        .discover
        .push_back(Ok(Some(NodeId::parse("node-1").unwrap())));

    assert_eq!(
        harness
            .manager
            .configured_node(view.profile_id)
            .await
            .unwrap(),
        Some("node-1".to_owned())
    );
    assert_eq!(harness.count("discover:"), 2);
    assert_eq!(harness.count("refresh:"), 1);
    let entries = harness.entries();
    let pending = entries
        .iter()
        .position(|entry| entry == "write_anchor:commit_pending")
        .unwrap();
    let refresh = entries
        .iter()
        .position(|entry| entry.starts_with("refresh:"))
        .unwrap();
    let ready = entries
        .iter()
        .rposition(|entry| entry == "write_anchor:ready")
        .unwrap();
    let retry = entries
        .iter()
        .rposition(|entry| entry.starts_with("discover:"))
        .unwrap();
    assert!(pending < refresh && refresh < ready && ready < retry);
    assert_eq!(harness.vault.anchor().unwrap().generation, 2);
    assert!(
        entries[retry].contains("02020202"),
        "retry uses the successor access credential"
    );
}

#[tokio::test]
async fn inventory_page_401_retries_once_with_the_same_node_and_cursor() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    harness
        .controllers
        .script()
        .containers
        .push_back(Err(NativeClientError::Unauthorized));
    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));
    harness
        .controllers
        .script()
        .containers
        .push_back(Ok(page()));
    let cursor = format!("v1:{}", "b".repeat(64));

    let result = harness
        .manager
        .container_inventory(view.profile_id, "node-1", Some(cursor.clone()), 25)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(result).unwrap()["items"][0]["displayName"],
        "web"
    );
    let requests: Vec<_> = harness
        .entries()
        .into_iter()
        .filter(|entry| entry.starts_with("containers:"))
        .collect();
    assert_eq!(requests.len(), 2);
    assert!(requests
        .iter()
        .all(|entry| entry.ends_with(&format!(":node-1:25:{cursor}"))));
    assert_eq!(harness.count("refresh:"), 1);
}

#[tokio::test]
async fn second_401_invalidates_locally_without_another_refresh() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    harness
        .controllers
        .script()
        .discover
        .push_back(Err(NativeClientError::Unauthorized));
    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));
    harness
        .controllers
        .script()
        .discover
        .push_back(Err(NativeClientError::Unauthorized));

    assert_eq!(
        harness.manager.configured_node(view.profile_id).await.err(),
        Some(InventoryError::ReloginRequired)
    );
    assert_eq!(harness.count("refresh:"), 1);
    assert_eq!(harness.count("discover:"), 2);
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::LoggedOut));
    assert_eq!(
        harness.manager.configured_node(view.profile_id).await.err(),
        Some(InventoryError::Unauthenticated)
    );
}

#[tokio::test]
async fn refresh_rejection_and_vault_outage_have_distinct_recovery_codes() {
    let rejected = Harness::new();
    let view = rejected.login_ok().await;
    rejected
        .controllers
        .script()
        .discover
        .push_back(Err(NativeClientError::Unauthorized));
    rejected
        .controllers
        .script()
        .refresh
        .push_back(Err(SessionError::ReloginRequired));
    assert_eq!(
        rejected
            .manager
            .configured_node(view.profile_id)
            .await
            .err(),
        Some(InventoryError::ReloginRequired)
    );
    assert_eq!(rejected.vault.anchor_state(), Some(AnchorState::LoggedOut));

    let outage = Harness::new();
    let view = outage.login_ok().await;
    outage
        .controllers
        .script()
        .discover
        .push_back(Err(NativeClientError::Unauthorized));
    outage.vault.fail_reads.store(true, Ordering::SeqCst);
    assert_eq!(
        outage.manager.configured_node(view.profile_id).await.err(),
        Some(InventoryError::VaultUnavailable)
    );
    assert_eq!(outage.count("refresh:"), 0);
    outage.vault.fail_reads.store(false, Ordering::SeqCst);
    outage
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));
    assert!(outage.manager.restore().await.unwrap().is_some());
}

#[tokio::test]
async fn cancelling_a_read_restores_access_and_leaves_no_work_running() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    let gate = Arc::new(Notify::new());
    harness.controllers.script().discover_gate = Some(gate.clone());
    harness
        .controllers
        .script()
        .discover
        .push_back(Ok(Some(NodeId::parse("node-1").unwrap())));
    let mut read = Box::pin(harness.manager.configured_node(view.profile_id));
    tokio::select! {
        result = &mut read => panic!("read unexpectedly completed: {result:?}"),
        () = tokio::time::sleep(Duration::from_millis(20)) => {}
    }
    assert_eq!(harness.count("discover:"), 1);
    drop(read);
    gate.notify_waiters();
    tokio::task::yield_now().await;
    assert_eq!(harness.count("discover_done"), 0);
    assert_eq!(harness.manager.restore().await.unwrap(), Some(view.clone()));
    harness.controllers.script().discover_gate = None;
    harness.controllers.script().discover.push_back(Ok(None));
    assert_eq!(
        harness
            .manager
            .configured_node(view.profile_id)
            .await
            .unwrap(),
        None
    );
    assert_eq!(harness.count("refresh:"), 0);
}

#[tokio::test]
async fn logout_waits_for_an_in_flight_inventory_read() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    let gate = Arc::new(Notify::new());
    harness.controllers.script().discover_gate = Some(gate.clone());
    harness.controllers.script().discover.push_back(Ok(None));
    harness.controllers.script().logout.push_back(Ok(()));

    let release = async {
        while harness.count("discover:") == 0 {
            tokio::task::yield_now().await;
        }
        assert_eq!(harness.count("logout:"), 0);
        gate.notify_waiters();
    };
    let (read, logout, ()) = tokio::join!(
        harness.manager.configured_node(view.profile_id),
        harness.manager.logout(),
        release,
    );
    assert_eq!(read.unwrap(), None);
    logout.unwrap();
    let entries = harness.entries();
    let read_done = entries
        .iter()
        .position(|entry| entry == "discover_done")
        .unwrap();
    let revoke = entries
        .iter()
        .position(|entry| entry.starts_with("logout:"))
        .unwrap();
    assert!(read_done < revoke);
}
