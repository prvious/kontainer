//! Service IPC contracts: list and detail views, actions under refresh, and quorum refusals.

use kontainer_native_client::{
    AdmittedOperation, NativeClientError, ServiceDetail, ServiceItem, ServiceList, ServiceMode,
    TaskItem,
};

use crate::session::manager::{InventoryError, ServiceDetailView, ServiceListView};

use super::fakes::{future_ms, issued, Harness};

const FIXTURE: &str = include_str!("../../../../tests/fixtures/services-ipc.json");
const LIB_SOURCE: &str = include_str!("../../lib.rs");
const KEY: &str = "018f2e34-0000-7000-8000-0000000000aa";
const WEB: &str = "webappweb0000000000000000";

/// A timestamp of whatever type the field wants (the client's `DateTime<Utc>`).
fn at<T: std::str::FromStr>(value: &str) -> Option<T>
where
    T::Err: std::fmt::Debug,
{
    Some(value.parse().unwrap())
}

fn web() -> ServiceItem {
    ServiceItem {
        id: WEB.to_owned(),
        name: "webapp_web".to_owned(),
        image: "nginx:alpine".to_owned(),
        image_reference: format!("nginx:alpine@sha256:{}", "a".repeat(64)),
        mode: ServiceMode::Replicated,
        desired_replicas: 3,
        running_replicas: 2,
        ports: vec!["8080:80/tcp".to_owned()],
        created_at: at("2026-09-20T10:00:00.000Z"),
        updated_at: at("2026-09-28T11:00:00.000Z"),
        version: 7,
        stack: Some("webapp".to_owned()),
        update_state: None,
        update_message: None,
    }
}

fn service_list() -> ServiceList {
    ServiceList {
        items: vec![
            ServiceItem {
                id: "monitoring000000000000000".to_owned(),
                name: "monitoring_collector".to_owned(),
                image: "traefik/whoami".to_owned(),
                image_reference: format!("traefik/whoami@sha256:{}", "c".repeat(64)),
                mode: ServiceMode::Global,
                desired_replicas: 3,
                running_replicas: 3,
                ports: Vec::new(),
                created_at: at("2026-09-20T10:00:00.000Z"),
                updated_at: at("2026-09-27T10:00:00.000Z"),
                version: 3,
                stack: Some("monitoring".to_owned()),
                update_state: None,
                update_message: None,
            },
            web(),
            ServiceItem {
                id: "toolboxalpine000000000000".to_owned(),
                name: "toolbox_alpine".to_owned(),
                image: "alpine:latest".to_owned(),
                image_reference: "alpine:latest".to_owned(),
                mode: ServiceMode::Replicated,
                desired_replicas: 1,
                running_replicas: 0,
                ports: Vec::new(),
                created_at: None,
                updated_at: None,
                version: 12,
                stack: Some("toolbox".to_owned()),
                update_state: Some("updating".to_owned()),
                update_message: Some("update in progress".to_owned()),
            },
        ],
        snapshot_at: "2026-09-28T12:05:00.000Z".parse().unwrap(),
    }
}

fn service_detail() -> ServiceDetail {
    ServiceDetail {
        service: web(),
        tasks: vec![TaskItem {
            id: "task00000000000000000000a".to_owned(),
            slot: Some(3),
            swarm_node_id: Some("swarmnode".to_owned()),
            state: "pending".to_owned(),
            desired_state: "running".to_owned(),
            error: Some("no suitable node (insufficient memory on 3 nodes)".to_owned()),
            message: Some("pending task scheduling".to_owned()),
            updated_at: at("2026-09-28T12:04:00.000Z"),
            container_id: None,
        }],
        latest_error: Some("no suitable node (insufficient memory on 3 nodes)".to_owned()),
    }
}

#[test]
fn service_views_and_commands_match_the_shared_fixture() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        serde_json::to_value(ServiceListView::from(service_list())).unwrap(),
        fixture["serviceList"]
    );
    assert_eq!(
        serde_json::to_value(ServiceDetailView::from(service_detail())).unwrap(),
        fixture["serviceDetail"]
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
async fn service_calls_carry_versions_retry_once_on_401_and_surface_quorum() {
    use kontainer_native_client::ServiceAction;

    let harness = Harness::new();
    let view = harness.login_ok().await;
    {
        let mut script = harness.controllers.script();
        script.services.push_back(Ok(service_list()));
        script
            .services
            .push_back(Err(NativeClientError::QuorumUnavailable {
                retry_after_seconds: 10,
            }));
        script.service_details.push_back(Ok(service_detail()));
        script
            .admissions
            .push_back(Err(NativeClientError::Unauthorized));
        script
            .refresh
            .push_back(Ok(issued(2, future_ms(), future_ms())));
        script.admissions.push_back(Ok(AdmittedOperation {
            operation_id: uuid::Uuid::from_u128(0xcc),
            replayed: false,
        }));
        script
            .admissions
            .push_back(Err(NativeClientError::VersionConflict));
    }
    let list = harness
        .manager
        .services(view.profile_id, Some("webapp"))
        .await
        .unwrap();
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(serde_json::to_value(list).unwrap(), fixture["serviceList"]);
    assert_eq!(
        harness.manager.services(view.profile_id, None).await.err(),
        Some(InventoryError::QuorumUnavailable)
    );
    let detail = harness.manager.service(view.profile_id, WEB).await.unwrap();
    assert_eq!(
        serde_json::to_value(detail).unwrap(),
        fixture["serviceDetail"]
    );

    harness
        .manager
        .service_action(
            view.profile_id,
            WEB,
            ServiceAction::Scale { replicas: 3 },
            7,
            KEY,
        )
        .await
        .unwrap();
    let actions: Vec<_> = harness
        .entries()
        .into_iter()
        .filter(|entry| entry.starts_with("service_action:"))
        .collect();
    assert_eq!(actions.len(), 2, "one 401, one retry with the same key");
    assert!(actions
        .iter()
        .all(|entry| entry.ends_with(&format!(":{WEB}:scale=3:7:{KEY}"))));
    assert_eq!(
        harness
            .manager
            .service_action(view.profile_id, WEB, ServiceAction::Restart, 7, KEY)
            .await
            .err(),
        Some(InventoryError::VersionConflict)
    );
    assert!(harness
        .entries()
        .iter()
        .any(|entry| entry.starts_with("services:") && entry.ends_with(":webapp")));
    assert_eq!(
        harness
            .manager
            .service(view.profile_id, "short")
            .await
            .err(),
        Some(InventoryError::InvalidRequest)
    );
    assert_eq!(harness.count("service:"), 1);
}
