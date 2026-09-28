//! Stack IPC contracts: the list, the removal plan, and removal under refresh.

use kontainer_native_client::{
    AdmittedOperation, NativeClientError, StackItem, StackList, StackRemovalPlan, StackStatus,
};

use crate::session::manager::{InventoryError, StackListView, StackPlanView};

use super::fakes::{future_ms, issued, Harness};

const FIXTURE: &str = include_str!("../../../../tests/fixtures/stacks-ipc.json");
const LIB_SOURCE: &str = include_str!("../../lib.rs");
const KEY: &str = "018f2e34-0000-7000-8000-0000000000aa";

fn at<T: std::str::FromStr>(value: &str) -> Option<T>
where
    T::Err: std::fmt::Debug,
{
    Some(value.parse().unwrap())
}

fn stack(name: &str, status: StackStatus, services: u32, running: u64, desired: u64) -> StackItem {
    StackItem {
        name: name.to_owned(),
        status,
        services,
        running_containers: running,
        desired_containers: desired,
        cpu_percent: None,
        memory_bytes: None,
        last_deployed: None,
    }
}

fn stack_list() -> StackList {
    StackList {
        items: vec![
            StackItem {
                cpu_percent: Some(1.25),
                memory_bytes: Some(41_943_040),
                last_deployed: at("2026-09-27T10:00:00.000Z"),
                ..stack("monitoring", StackStatus::Running, 2, 4, 4)
            },
            StackItem {
                cpu_percent: Some(0.5),
                memory_bytes: Some(20_971_520),
                last_deployed: at("2026-09-28T11:00:00.000Z"),
                ..stack("toolbox", StackStatus::Partial, 3, 4, 5)
            },
            stack("idle", StackStatus::Stopped, 1, 0, 1),
        ],
        snapshot_at: "2026-09-28T12:05:00.000Z".parse().unwrap(),
    }
}

fn plan() -> StackRemovalPlan {
    StackRemovalPlan {
        services: vec![
            "toolbox_alpine".to_owned(),
            "toolbox_nginx".to_owned(),
            "toolbox_redis".to_owned(),
        ],
        networks: vec!["toolbox_kontainer".to_owned()],
        volumes_retained: vec!["toolbox_redis-data".to_owned()],
    }
}

#[test]
fn stack_views_and_commands_match_the_shared_fixture() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        serde_json::to_value(StackListView::from(stack_list())).unwrap(),
        fixture["stackList"]
    );
    assert_eq!(
        serde_json::to_value(StackPlanView::from(plan())).unwrap(),
        fixture["plan"]
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
async fn stack_calls_carry_the_name_and_retry_removal_once_on_401() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    {
        let mut script = harness.controllers.script();
        script.stacks.push_back(Ok(stack_list()));
        script.plans.push_back(Ok(plan()));
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
    }
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let list = harness.manager.stacks(view.profile_id).await.unwrap();
    assert_eq!(serde_json::to_value(list).unwrap(), fixture["stackList"]);
    let plan = harness
        .manager
        .stack_plan(view.profile_id, "toolbox")
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(plan).unwrap(), fixture["plan"]);
    harness
        .manager
        .remove_stack(view.profile_id, "toolbox", KEY)
        .await
        .unwrap();
    let removals: Vec<_> = harness
        .entries()
        .into_iter()
        .filter(|entry| entry.starts_with("remove_stack:"))
        .collect();
    assert_eq!(removals.len(), 2, "one 401, one retry with the same key");
    assert!(removals
        .iter()
        .all(|entry| entry.ends_with(&format!(":toolbox:{KEY}"))));
    assert_eq!(
        harness
            .manager
            .stack_plan(view.profile_id, "-bad")
            .await
            .err(),
        Some(InventoryError::InvalidRequest)
    );
    assert_eq!(harness.count("stack_plan:"), 1);
}
