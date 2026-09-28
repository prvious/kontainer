//! Authenticated read IPC contracts and custody behavior under read, refresh, and cancellation.

use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use kontainer_native_client::{
    ContainerItem, ContainerPage, ContainerState, ControllerSettings, NativeClientError, NodeId,
    NodeList, NodeSample, NodeStatus, NodeSummary, SamplePoint, SampleSeries,
};
use tokio::sync::Notify;

use crate::session::{
    errors::SessionError,
    manager::{InventoryError, NodeListView},
    models::AnchorState,
};

use super::fakes::{future_ms, issued, Harness, PROFILE_ID};

const FIXTURE: &str = include_str!("../../../../tests/fixtures/inventory-ipc.json");
const NODES_FIXTURE: &str = include_str!("../../../../tests/fixtures/nodes-ipc.json");
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

fn node(id: &str, status: NodeStatus, cores: u32, memory: u64) -> NodeSummary {
    let (started, first, last) = if id == "manager" {
        (
            "2026-09-28T11:58:00.000Z",
            "2026-09-28T11:58:01.000Z",
            "2026-09-28T12:00:00.000Z",
        )
    } else {
        (
            "2026-09-28T11:00:00.000Z",
            "2026-09-28T11:00:01.000Z",
            "2026-09-28T11:30:00.000Z",
        )
    };
    NodeSummary {
        id: NodeId::parse(id).unwrap(),
        hostname: id.to_owned(),
        status,
        swarm_node_id: (id == "manager").then(|| "tj42rq5nzwf8".to_owned()),
        role: None,
        availability: None,
        agent_version: "0.1.0".to_owned(),
        docker_version: "29.8.0".to_owned(),
        os: "Alpine Linux v3.22".to_owned(),
        arch: "aarch64".to_owned(),
        cpu_cores: cores,
        memory_total_bytes: memory,
        agent_started_at: started.parse().unwrap(),
        first_seen_at: first.parse().unwrap(),
        last_check_in_at: last.parse().unwrap(),
        latest_sample: None,
    }
}

fn node_list() -> NodeList {
    let mut manager = node("manager", NodeStatus::Online, 4, 8 << 30);
    manager.latest_sample = Some(NodeSample {
        sampled_at: "2026-09-28T12:00:00.000Z".parse().unwrap(),
        cpu_percent: 12.5,
        memory_used_bytes: 1 << 30,
        disk_used_bytes: Some(5 << 30),
        containers_running: 5,
        containers_paused: 0,
        containers_exited: 1,
        net_rx_bytes_per_sec: Some(2048.0),
        net_tx_bytes_per_sec: None,
    });
    NodeList {
        items: vec![manager, node("worker-2", NodeStatus::Offline, 2, 4 << 30)],
        snapshot_at: "2026-09-28T12:00:02.000Z".parse().unwrap(),
    }
}

fn empty_nodes() -> NodeList {
    NodeList {
        items: Vec::new(),
        snapshot_at: "2026-09-28T12:00:02.000Z".parse().unwrap(),
    }
}

fn series() -> SampleSeries {
    SampleSeries {
        window: "5m".to_owned(),
        bucket: "60s".to_owned(),
        points: vec![
            SamplePoint {
                at: "2026-09-28T11:59:00.000Z".parse().unwrap(),
                cpu_percent: None,
                memory_used_bytes: None,
                disk_used_bytes: None,
                containers_running: None,
                net_rx_bytes_per_sec: None,
                net_tx_bytes_per_sec: None,
                nodes: 0,
            },
            SamplePoint {
                at: "2026-09-28T12:00:00.000Z".parse().unwrap(),
                cpu_percent: Some(30.0),
                memory_used_bytes: Some(1200.0),
                disk_used_bytes: Some(2000.0),
                containers_running: Some(4.0),
                net_rx_bytes_per_sec: Some(20.0),
                net_tx_bytes_per_sec: None,
                nodes: 2,
            },
        ],
        snapshot_at: "2026-09-28T12:00:30.000Z".parse().unwrap(),
    }
}

fn node_ids(view: NodeListView) -> Vec<String> {
    serde_json::to_value(view).unwrap()["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn inventory_ipc_fields_codes_and_commands_match_shared_fixture() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        serde_json::to_value(super::super::manager::ContainerPageView::from(page())).unwrap(),
        fixture["containerPage"]
    );
    let codes: Vec<_> = InventoryError::ALL
        .iter()
        .map(|code| serde_json::to_value(code).unwrap())
        .collect();
    assert_eq!(serde_json::to_value(codes).unwrap(), fixture["errorCodes"]);
    for command in fixture["commands"].as_array().unwrap() {
        let name = command.as_str().unwrap();
        assert!(LIB_SOURCE.contains(&format!("actions::inventory::{name}::{name}")));
    }
    assert!(!LIB_SOURCE.contains("get_configured_node"));
}

#[test]
fn node_and_settings_ipc_views_match_shared_fixture() {
    let fixture: serde_json::Value = serde_json::from_str(NODES_FIXTURE).unwrap();
    assert_eq!(
        serde_json::to_value(NodeListView::from(node_list())).unwrap(),
        fixture["nodeList"]
    );
    assert_eq!(
        serde_json::to_value(super::super::manager::SampleSeriesView::from(series())).unwrap(),
        fixture["clusterSamples"]
    );
    assert_eq!(
        serde_json::to_value(super::super::manager::ControllerSettingsView::from(
            ControllerSettings {
                metrics_retention_days: 7
            }
        ))
        .unwrap(),
        fixture["settings"]
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

#[test]
fn native_failures_map_to_specific_closed_ipc_codes() {
    for (native, expected) in [
        (
            NativeClientError::InvalidPagination,
            InventoryError::InvalidPagination,
        ),
        (
            NativeClientError::InvalidRequest,
            InventoryError::InvalidRequest,
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
async fn reads_use_only_the_active_profile_and_serialize_their_views() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    harness
        .controllers
        .script()
        .nodes
        .push_back(Ok(node_list()));
    harness
        .controllers
        .script()
        .containers
        .push_back(Ok(page()));
    harness.controllers.script().samples.push_back(Ok(series()));
    harness.controllers.script().samples.push_back(Ok(series()));
    harness
        .controllers
        .script()
        .settings
        .push_back(Ok(ControllerSettings {
            metrics_retention_days: 7,
        }));
    harness
        .controllers
        .script()
        .settings
        .push_back(Ok(ControllerSettings {
            metrics_retention_days: 30,
        }));

    assert_eq!(
        node_ids(harness.manager.nodes(view.profile_id).await.unwrap()),
        vec!["manager", "worker-2"]
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
    harness
        .manager
        .samples(view.profile_id, None, "5m", "60s")
        .await
        .unwrap();
    harness
        .manager
        .samples(view.profile_id, Some("manager"), "1h", "60s")
        .await
        .unwrap();
    let settings = harness
        .manager
        .controller_settings(view.profile_id)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(settings).unwrap()["metricsRetentionDays"],
        7
    );
    let updated = harness
        .manager
        .update_controller_settings(view.profile_id, 30)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(updated).unwrap()["metricsRetentionDays"],
        30
    );

    assert_eq!(harness.count("nodes:"), 1);
    assert_eq!(harness.count("containers:"), 1);
    let samples: Vec<_> = harness
        .entries()
        .into_iter()
        .filter(|entry| entry.starts_with("samples:"))
        .collect();
    assert_eq!(samples.len(), 2);
    assert!(samples[0].ends_with(":cluster:5m:60s"));
    assert!(samples[1].ends_with(":manager:1h:60s"));
    assert_eq!(harness.count("update_settings:"), 1);
    assert!(harness
        .entries()
        .iter()
        .any(|entry| entry.starts_with("update_settings:") && entry.ends_with(":30")));
    assert_eq!(harness.count("refresh:"), 0);

    assert_eq!(
        harness.manager.nodes(PROFILE_ID).await.err(),
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
    assert_eq!(
        harness
            .manager
            .samples(view.profile_id, Some("../bad"), "5m", "60s")
            .await
            .err(),
        Some(InventoryError::InvalidNodeId)
    );
    assert_eq!(harness.count("nodes:"), 1);
    assert_eq!(harness.count("containers:"), 1);
    assert_eq!(harness.count("samples:"), 2);
}

#[tokio::test]
async fn one_401_rotates_in_order_then_retries_the_same_read_once() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    harness
        .controllers
        .script()
        .nodes
        .push_back(Err(NativeClientError::Unauthorized));
    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));
    harness
        .controllers
        .script()
        .nodes
        .push_back(Ok(node_list()));

    assert_eq!(
        node_ids(harness.manager.nodes(view.profile_id).await.unwrap()),
        vec!["manager", "worker-2"]
    );
    assert_eq!(harness.count("nodes:"), 2);
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
        .rposition(|entry| entry.starts_with("nodes:"))
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
async fn a_settings_write_401_retries_with_the_same_value() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    harness
        .controllers
        .script()
        .settings
        .push_back(Err(NativeClientError::Unauthorized));
    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));
    harness
        .controllers
        .script()
        .settings
        .push_back(Ok(ControllerSettings {
            metrics_retention_days: 14,
        }));
    harness
        .manager
        .update_controller_settings(view.profile_id, 14)
        .await
        .unwrap();
    let writes: Vec<_> = harness
        .entries()
        .into_iter()
        .filter(|entry| entry.starts_with("update_settings:"))
        .collect();
    assert_eq!(writes.len(), 2);
    assert!(writes.iter().all(|entry| entry.ends_with(":14")));
}

#[tokio::test]
async fn second_401_invalidates_locally_without_another_refresh() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    harness
        .controllers
        .script()
        .nodes
        .push_back(Err(NativeClientError::Unauthorized));
    harness
        .controllers
        .script()
        .refresh
        .push_back(Ok(issued(2, future_ms(), future_ms())));
    harness
        .controllers
        .script()
        .nodes
        .push_back(Err(NativeClientError::Unauthorized));

    assert_eq!(
        harness.manager.nodes(view.profile_id).await.err(),
        Some(InventoryError::ReloginRequired)
    );
    assert_eq!(harness.count("refresh:"), 1);
    assert_eq!(harness.count("nodes:"), 2);
    assert_eq!(harness.vault.anchor_state(), Some(AnchorState::LoggedOut));
    assert_eq!(
        harness.manager.nodes(view.profile_id).await.err(),
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
        .nodes
        .push_back(Err(NativeClientError::Unauthorized));
    rejected
        .controllers
        .script()
        .refresh
        .push_back(Err(SessionError::ReloginRequired));
    assert_eq!(
        rejected.manager.nodes(view.profile_id).await.err(),
        Some(InventoryError::ReloginRequired)
    );
    assert_eq!(rejected.vault.anchor_state(), Some(AnchorState::LoggedOut));

    let outage = Harness::new();
    let view = outage.login_ok().await;
    outage
        .controllers
        .script()
        .nodes
        .push_back(Err(NativeClientError::Unauthorized));
    outage.vault.fail_reads.store(true, Ordering::SeqCst);
    assert_eq!(
        outage.manager.nodes(view.profile_id).await.err(),
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
    harness.controllers.script().nodes_gate = Some(gate.clone());
    harness
        .controllers
        .script()
        .nodes
        .push_back(Ok(node_list()));
    let mut read = Box::pin(harness.manager.nodes(view.profile_id));
    tokio::select! {
        result = &mut read => panic!("read unexpectedly completed: {result:?}"),
        () = tokio::time::sleep(Duration::from_millis(20)) => {}
    }
    assert_eq!(harness.count("nodes:"), 1);
    drop(read);
    gate.notify_waiters();
    tokio::task::yield_now().await;
    assert_eq!(harness.count("nodes_done"), 0);
    assert_eq!(harness.manager.restore().await.unwrap(), Some(view.clone()));
    harness.controllers.script().nodes_gate = None;
    harness
        .controllers
        .script()
        .nodes
        .push_back(Ok(empty_nodes()));
    assert!(node_ids(harness.manager.nodes(view.profile_id).await.unwrap()).is_empty());
    assert_eq!(harness.count("refresh:"), 0);
}

#[tokio::test]
async fn logout_waits_for_an_in_flight_read() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    let gate = Arc::new(Notify::new());
    harness.controllers.script().nodes_gate = Some(gate.clone());
    harness
        .controllers
        .script()
        .nodes
        .push_back(Ok(empty_nodes()));
    harness.controllers.script().logout.push_back(Ok(()));

    let release = async {
        while harness.count("nodes:") == 0 {
            tokio::task::yield_now().await;
        }
        assert_eq!(harness.count("logout:"), 0);
        gate.notify_waiters();
    };
    let (read, logout, ()) = tokio::join!(
        harness.manager.nodes(view.profile_id),
        harness.manager.logout(),
        release,
    );
    assert!(node_ids(read.unwrap()).is_empty());
    logout.unwrap();
    let entries = harness.entries();
    let read_done = entries
        .iter()
        .position(|entry| entry == "nodes_done")
        .unwrap();
    let revoke = entries
        .iter()
        .position(|entry| entry.starts_with("logout:"))
        .unwrap();
    assert!(read_done < revoke);
}
