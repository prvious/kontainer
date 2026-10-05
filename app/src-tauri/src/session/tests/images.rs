//! Image IPC contracts: the list view, removal targets, and pulls under refresh.

use kontainer_native_client::{
    AdmittedOperation, ImageItem, ImageList, NativeClientError, NodeId, NodeOutcome,
    NodeOutcomeStatus,
};

use crate::session::manager::{ImageListView, ImageNodesInput, InventoryError};

use super::fakes::{future_ms, issued, Harness};

const FIXTURE: &str = include_str!("../../../../tests/fixtures/images-ipc.json");
const LIB_SOURCE: &str = include_str!("../../lib.rs");
const KEY: &str = "018f2e34-0000-7000-8000-0000000000aa";

fn image_id(fill: char) -> String {
    format!("sha256:{}", fill.to_string().repeat(64))
}

fn image_list() -> ImageList {
    let nodes = |names: &[&str]| {
        names
            .iter()
            .map(|name| NodeId::parse(name).unwrap())
            .collect()
    };
    let outcome = |node: &str, status| NodeOutcome {
        node_id: NodeId::parse(node).unwrap(),
        status,
        message: None,
    };
    ImageList {
        items: vec![
            ImageItem {
                image_id: image_id('a'),
                repo_tags: vec!["nginx:1.27".to_owned()],
                repo_digests: Vec::new(),
                size_bytes: 192_000_000,
                created_at: "2026-09-01T10:00:00.000Z".parse().unwrap(),
                used_by: 3,
                node_ids: nodes(&["manager", "worker-1"]),
            },
            ImageItem {
                image_id: image_id('c'),
                repo_tags: Vec::new(),
                repo_digests: Vec::new(),
                size_bytes: 7_800_000,
                created_at: "2026-08-20T08:30:00.000Z".parse().unwrap(),
                used_by: 0,
                node_ids: nodes(&["manager"]),
            },
        ],
        nodes: vec![
            outcome("manager", NodeOutcomeStatus::Ok),
            outcome("worker-1", NodeOutcomeStatus::Ok),
            outcome("worker-2", NodeOutcomeStatus::Unavailable),
        ],
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
fn image_views_and_commands_match_the_shared_fixture() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        serde_json::to_value(ImageListView::from(image_list())).unwrap(),
        fixture["imageList"]
    );
    for command in fixture["commands"].as_array().unwrap() {
        let domain = command[0].as_str().unwrap();
        let name = command[1].as_str().unwrap();
        assert!(
            LIB_SOURCE.contains(&format!("actions::{domain}::{name}::{name}")),
            "{name} is registered"
        );
    }
    let all: ImageNodesInput = serde_json::from_value(serde_json::json!("all")).unwrap();
    assert!(matches!(all, ImageNodesInput::All(_)));
    let some: ImageNodesInput = serde_json::from_value(serde_json::json!(["manager"])).unwrap();
    assert!(matches!(some, ImageNodesInput::Only(ref nodes) if nodes == &["manager"]));
    assert!(
        serde_json::from_value::<ImageNodesInput>(serde_json::json!("every")).is_err(),
        "only the literal \"all\" selects every node"
    );
}

#[tokio::test]
async fn image_reads_and_mutations_carry_their_targets_and_retry_once_on_401() {
    let harness = Harness::new();
    let view = harness.login_ok().await;
    {
        let mut script = harness.controllers.script();
        script.images.push_back(Ok(image_list()));
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
    let list = harness.manager.images(view.profile_id).await.unwrap();
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(serde_json::to_value(list).unwrap(), fixture["imageList"]);

    let removal = harness
        .manager
        .remove_image(
            view.profile_id,
            &image_id('a'),
            &ImageNodesInput::Only(vec!["worker-1".to_owned()]),
            true,
            KEY,
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(removal).unwrap()["operationId"],
        uuid::Uuid::from_u128(0xcc).to_string()
    );
    let removals: Vec<_> = harness
        .entries()
        .into_iter()
        .filter(|entry| entry.starts_with("remove_image:"))
        .collect();
    assert_eq!(removals.len(), 2, "one 401, one retry with the same key");
    let expected = format!(":{}:worker-1:true:{KEY}", image_id('a'));
    assert!(removals.iter().all(|entry| entry.ends_with(&expected)));
    assert_eq!(harness.count("refresh:"), 1);

    let all: ImageNodesInput = serde_json::from_value(serde_json::json!("all")).unwrap();
    harness
        .manager
        .remove_image(view.profile_id, &image_id('a'), &all, false, KEY)
        .await
        .unwrap();
    assert!(harness
        .entries()
        .iter()
        .any(|entry| entry.ends_with(&format!(":{}:all:false:{KEY}", image_id('a')))));

    harness
        .manager
        .pull_image(
            view.profile_id,
            "alpine:3.20",
            &["manager".to_owned(), "worker-1".to_owned()],
            KEY,
        )
        .await
        .unwrap();
    assert!(harness
        .entries()
        .iter()
        .any(|entry| entry.starts_with("pull_image:")
            && entry.ends_with(&format!(":alpine:3.20:manager,worker-1:{KEY}"))));

    // Invalid input never leaves the app.
    for (reference, nodes, key) in [
        ("alpine", vec!["bad/node".to_owned()], KEY),
        ("alpine", vec!["manager".to_owned()], "short"),
    ] {
        assert!(harness
            .manager
            .pull_image(view.profile_id, reference, &nodes, key)
            .await
            .is_err());
    }
    assert_eq!(
        harness
            .manager
            .remove_image(
                view.profile_id,
                &image_id('a'),
                &ImageNodesInput::Only(vec!["..".to_owned()]),
                false,
                KEY
            )
            .await
            .err(),
        Some(InventoryError::InvalidNodeId)
    );
    assert_eq!(harness.count("pull_image:"), 1);
    assert_eq!(harness.count("remove_image:"), 3);
    assert_eq!(harness.count("images:"), 1);
}
