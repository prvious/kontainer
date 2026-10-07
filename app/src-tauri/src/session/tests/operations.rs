//! Operation and admission IPC views, and operation reads through the session.

use kontainer_native_client::{
    AdmittedOperation, NativeClientError, OperationSnapshot, OperationState,
};
use uuid::Uuid;

use crate::session::manager::{AdmittedView, InventoryError, OperationView};

use super::fakes::Harness;

const FIXTURE: &str = include_str!("../../../../tests/fixtures/operations-ipc.json");
const LIB_SOURCE: &str = include_str!("../../lib.rs");
const RUNNING_ID: &str = "018f2e34-0000-7000-8000-0000000000cc";

fn fixture() -> serde_json::Value {
    serde_json::from_str(FIXTURE).unwrap()
}

fn running() -> OperationSnapshot {
    OperationSnapshot {
        id: Uuid::parse_str(RUNNING_ID).unwrap(),
        kind: "noop".to_owned(),
        state: OperationState::Running,
        recoverable: false,
        uncertain: false,
        last_error_code: None,
        created_at: "2026-09-28T12:00:00.000Z".parse().unwrap(),
        updated_at: "2026-09-28T12:00:01.000Z".parse().unwrap(),
        error_message: None,
        progress: Some(serde_json::json!({"total": 3, "done": 1})),
        started_at: Some("2026-09-28T12:00:00.500Z".parse().unwrap()),
        finished_at: None,
        request_id: Some("01J9ZM2R".to_owned()),
    }
}

fn failed() -> OperationSnapshot {
    OperationSnapshot {
        id: Uuid::parse_str("018f2e34-0000-7000-8000-0000000000dd").unwrap(),
        kind: "service.scale".to_owned(),
        state: OperationState::Failed,
        last_error_code: Some("no_eligible_nodes".to_owned()),
        updated_at: "2026-09-28T12:00:09.000Z".parse().unwrap(),
        error_message: Some("No node satisfies the placement constraints.".to_owned()),
        progress: None,
        finished_at: Some("2026-09-28T12:00:09.000Z".parse().unwrap()),
        ..running()
    }
}

#[test]
fn operation_views_match_the_shared_fixture_and_commands_are_registered() {
    let fixture = fixture();
    assert_eq!(
        serde_json::to_value(OperationView::from(running())).unwrap(),
        fixture["operation"]
    );
    assert_eq!(
        serde_json::to_value(OperationView::from(failed())).unwrap(),
        fixture["failedOperation"]
    );
    assert_eq!(
        serde_json::to_value(AdmittedView::from(AdmittedOperation {
            operation_id: Uuid::parse_str(RUNNING_ID).unwrap(),
            replayed: false,
        }))
        .unwrap(),
        fixture["admitted"]
    );
    for command in fixture["commands"].as_array().unwrap() {
        let name = command.as_str().unwrap();
        assert!(LIB_SOURCE.contains(&format!("actions::operations::{name}::{name}")));
    }
}

#[tokio::test]
async fn operations_read_poll_and_cancel_through_the_session() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    {
        let mut script = harness.controllers.script();
        script.operations.push_back(Ok(running()));
        script
            .operations
            .push_back(Err(NativeClientError::Conflict));
        script
            .operations
            .push_back(Err(NativeClientError::NotFound));
    }
    let polled = harness
        .manager
        .operation(view.profile_id, RUNNING_ID)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(polled).unwrap(),
        fixture()["operation"]
    );
    assert_eq!(
        harness
            .manager
            .cancel_operation(view.profile_id, RUNNING_ID)
            .await
            .err(),
        Some(InventoryError::Conflict)
    );
    assert_eq!(
        harness
            .manager
            .operation(view.profile_id, RUNNING_ID)
            .await
            .err(),
        Some(InventoryError::NotFound),
        "an operation read never reports node_not_found"
    );
    assert_eq!(
        harness
            .manager
            .operation(view.profile_id, "not-a-uuid")
            .await
            .err(),
        Some(InventoryError::InvalidRequest)
    );
}
