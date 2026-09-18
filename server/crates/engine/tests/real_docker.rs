#![cfg(feature = "real-engine-tests")]
#![forbid(unsafe_code)]

use std::{env, path::PathBuf, time::Duration};

use kontainer_engine::{
    ContainerEngine, ContainerPageRequest, EngineAccessScope, EngineApiVersion, EngineErrorKind,
    LocalDockerEngine, LocalEngineOptions,
};
use tokio_util::sync::CancellationToken;

const TEST_SOCKET_ENV: &str = "KONTAINER_TEST_DOCKER_SOCKET";
const TEST_ENGINE_ENV: &str = "KONTAINER_TEST_ENGINE_IMAGE";
const TEST_VERSION_ENV: &str = "KONTAINER_TEST_ENGINE_VERSION";
const TEST_API_VERSION_ENV: &str = "KONTAINER_TEST_ENGINE_API_VERSION";

#[tokio::test]
async fn qualifies_configured_real_engine() {
    let socket_path = required_environment_path(TEST_SOCKET_ENV);
    let fixture_image = required_environment(TEST_ENGINE_ENV);
    let expected_version = required_environment(TEST_VERSION_ENV);
    let expected_api_version = required_environment(TEST_API_VERSION_ENV)
        .parse::<EngineApiVersion>()
        .expect("configured expected Engine API version must be valid");
    let options = LocalEngineOptions {
        socket_path,
        operation_timeout: Duration::from_secs(10),
    };
    let manager = LocalDockerEngine::connect_for_manager(options.clone()).unwrap();
    let node = LocalDockerEngine::connect_for_node(options).unwrap();
    let cancellation = CancellationToken::new();

    let manager_description = manager.describe(&cancellation).await.unwrap();
    let node_description = node.describe(&cancellation).await.unwrap();

    assert_eq!(manager_description.access_scope, EngineAccessScope::Manager);
    assert_eq!(node_description.access_scope, EngineAccessScope::NodeLocal);
    assert_eq!(manager_description.daemon_version, expected_version);
    assert_eq!(node_description.daemon_version, expected_version);
    assert_eq!(
        manager_description.negotiated_api_version,
        expected_api_version
    );
    assert_eq!(
        manager_description.negotiated_api_version,
        node_description.negotiated_api_version
    );
    assert!(manager_description.negotiated_api_version >= manager_description.minimum_api_version);
    assert!(manager_description.negotiated_api_version <= manager_description.daemon_api_version);
    assert!(manager.is_available().await.unwrap());
    assert!(node.is_available().await.unwrap());

    println!(
        "KONTAINER_ENGINE_EVIDENCE fixture_image={fixture_image} daemon_version={} \
         daemon_api={} minimum_api={} negotiated_api={} os={} architecture={} \
         manager_scope={:?} node_scope={:?} capabilities={:?}",
        manager_description.daemon_version,
        manager_description.daemon_api_version,
        manager_description.minimum_api_version,
        manager_description.negotiated_api_version,
        manager_description.operating_system,
        manager_description.architecture,
        manager_description.access_scope,
        node_description.access_scope,
        manager_description.capabilities,
    );
}

#[tokio::test]
async fn qualifies_paged_container_inventory() {
    use kontainer_engine::{ContainerState, is_valid_container_id};

    const RUNNING_FIXTURE: &str = "kontainer-inventory-running";
    const EXITED_FIXTURE: &str = "kontainer-inventory-exited";

    let socket_path = required_environment_path(TEST_SOCKET_ENV);
    let fixture_image = required_environment(TEST_ENGINE_ENV);
    let engine = LocalDockerEngine::connect_for_node(LocalEngineOptions {
        socket_path,
        operation_timeout: Duration::from_secs(10),
    })
    .unwrap();
    let cancellation = CancellationToken::new();
    let description = engine.describe(&cancellation).await.unwrap();

    // Page size one forces real multi-page traversal through `before`
    // cursors on the live daemon.
    let mut page_token = String::new();
    let mut collected = Vec::new();
    for _ in 0..1000 {
        let page = engine
            .list_containers(
                ContainerPageRequest::new(1, &page_token).unwrap(),
                &cancellation,
            )
            .await
            .unwrap();
        assert!(page.containers.len() <= 1);
        collected.extend(page.containers);
        match page.next_page_token {
            Some(next) => page_token = next,
            None => break,
        }
    }

    let running = collected
        .iter()
        .filter(|summary| summary.display_name == RUNNING_FIXTURE)
        .collect::<Vec<_>>();
    let exited = collected
        .iter()
        .filter(|summary| summary.display_name == EXITED_FIXTURE)
        .collect::<Vec<_>>();
    assert_eq!(running.len(), 1, "running fixture must appear exactly once");
    assert_eq!(exited.len(), 1, "exited fixture must appear exactly once");
    assert_eq!(running[0].state, ContainerState::Running);
    assert_eq!(exited[0].state, ContainerState::Exited);

    for summary in &collected {
        assert!(
            is_valid_container_id(&summary.container_id),
            "container_id must be a full ID"
        );
        assert!(!summary.display_name.trim().is_empty());
        assert!(!summary.image_reference.trim().is_empty());
        assert!(summary.created_at_seconds >= 0);
        assert!(summary.status.len() <= 256);
    }
    let mut ids = collected
        .iter()
        .map(|summary| summary.container_id.clone())
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), collected.len(), "paged walk must not duplicate");

    println!(
        "KONTAINER_CONTAINER_EVIDENCE fixture_image={fixture_image} daemon_version={} \
         daemon_api={} negotiated_api={} os={} architecture={} page_size=1 \
         total_collected={} running={}({:?}) exited={}({:?})",
        description.daemon_version,
        description.daemon_api_version,
        description.negotiated_api_version,
        description.operating_system,
        description.architecture,
        collected.len(),
        running[0].container_id,
        running[0].state,
        exited[0].container_id,
        exited[0].state,
    );
}

#[tokio::test]
async fn stopped_real_engine_maps_to_connection_unavailable() {
    let socket_path = required_environment_path(TEST_SOCKET_ENV);
    let options = LocalEngineOptions {
        socket_path,
        operation_timeout: Duration::from_secs(2),
    };

    match LocalDockerEngine::connect_for_node(options) {
        Ok(engine) => {
            let error = engine.is_available().await.unwrap_err();
            assert_eq!(error.kind(), EngineErrorKind::ConnectionUnavailable);
        }
        Err(error) => assert_eq!(error.kind(), EngineErrorKind::ConnectionUnavailable),
    }
}

fn required_environment(name: &str) -> String {
    env::var(name)
        .unwrap_or_else(|_| panic!("required test environment variable {name} is missing"))
}

fn required_environment_path(name: &str) -> PathBuf {
    required_environment(name).into()
}
