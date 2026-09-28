//! Stack list, removal plan, and removal contracts.

use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::support::{
    TestServer, assert_openapi_snapshot, assert_schema_fields, covered_contract, openapi_artifact,
    plain_client, respond, round_trip_keys,
};
use crate::credentials::AccessCredential;
use crate::error::NativeClientError;
use crate::operations::IdempotencyKey;
use crate::stacks::{StackListBody, StackName, StackStatus};

const STACK_LIST: &str = include_str!("fixtures/stacks/stack-list.json");
const ACCESS: &str = "kta_11111111111111111111111111111111.2222222222222222222222222222222222222222222222222222222222222222";
const OPERATION_ID: &str = "018f2e34-0000-7000-8000-0000000000cc";
const KEY: &str = "018f2e34-0000-7000-8000-0000000000aa";

fn access() -> AccessCredential {
    AccessCredential::parse(ACCESS).unwrap()
}

#[tokio::test]
async fn stacks_list_plan_and_remove_use_the_documented_routes() {
    let plan = json!({"services": ["toolbox_nginx", "toolbox_redis"], "networks": ["toolbox_kontainer"], "volumesRetained": ["toolbox_redis-data"]}).to_string();
    let admitted = json!({"operationId": OPERATION_ID, "replayed": false}).to_string();
    let server = TestServer::plain(vec![
        respond(200, STACK_LIST),
        respond(200, &plan),
        respond(202, &admitted),
    ])
    .await;
    let client = plain_client(&server);
    let stacks = client
        .list_stacks(&access(), &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(stacks.items[0].status, StackStatus::Running);
    assert_eq!(
        (
            stacks.items[0].running_containers,
            stacks.items[0].desired_containers
        ),
        (5, 5)
    );
    assert_eq!(stacks.items[1].cpu_percent, None);
    let name = StackName::parse("toolbox").unwrap();
    let removal = client
        .stack_removal_plan(&access(), &name, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(removal.volumes_retained, vec!["toolbox_redis-data"]);
    client
        .remove_stack(
            &access(),
            &name,
            &IdempotencyKey::parse(KEY).unwrap(),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    let requests = server.requests();
    assert_eq!(requests[0].target, "/api/v1/stacks");
    assert_eq!(requests[1].target, "/api/v1/stacks/toolbox/plan");
    assert_eq!(
        (requests[2].method.as_str(), requests[2].target.as_str()),
        ("DELETE", "/api/v1/stacks/toolbox")
    );
    assert_eq!(requests[2].header("idempotency-key"), Some(KEY));
    for bad in ["", "-bad", "sl/ash", &"a".repeat(129)] {
        assert_eq!(
            StackName::parse(bad).err(),
            Some(NativeClientError::InvalidRequest),
            "{bad:?}"
        );
    }
}

#[tokio::test]
async fn stack_lists_that_break_the_contract_are_rejected() {
    let valid: serde_json::Value = serde_json::from_str(STACK_LIST).unwrap();
    let mut unknown_status = valid.clone();
    unknown_status["items"][0]["status"] = json!("exploded");
    let mut extra = valid.clone();
    extra["items"][0]["composeFile"] = json!("/secret/canary.yml");
    let mut negative = valid.clone();
    negative["items"][0]["cpuPercent"] = json!(-1.0);
    for (label, body) in [
        ("an unknown status", unknown_status),
        ("an unknown field", extra),
        ("a negative figure", negative),
    ] {
        let server = TestServer::plain(vec![respond(200, &body.to_string())]).await;
        assert_eq!(
            plain_client(&server)
                .list_stacks(&access(), &CancellationToken::new())
                .await
                .err(),
            Some(NativeClientError::InvalidResponse),
            "{label}"
        );
    }
}

const STACK_OPENAPI_PATHS: [(&str, &str); 3] = [
    ("/api/v1/stacks", "get"),
    ("/api/v1/stacks/{name}/plan", "get"),
    ("/api/v1/stacks/{name}", "delete"),
];
const STACK_OPENAPI_SCHEMAS: [&str; 3] =
    ["StackListResponse", "StackResponse", "StackPlanResponse"];

#[test]
fn openapi_stacks_contract_matches_the_checked_artifact() {
    let document = openapi_artifact();
    assert_openapi_snapshot(
        "stacks/openapi-contract.json",
        &covered_contract(&document, &STACK_OPENAPI_PATHS, &STACK_OPENAPI_SCHEMAS),
    );
    assert_schema_fields(
        &document,
        "StackListResponse",
        round_trip_keys::<StackListBody>(STACK_LIST),
    );
    let list: serde_json::Value = serde_json::from_str(STACK_LIST).unwrap();
    assert_schema_fields(
        &document,
        "StackResponse",
        list["items"][0]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect(),
    );
}
