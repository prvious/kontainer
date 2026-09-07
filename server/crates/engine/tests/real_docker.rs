#![cfg(feature = "real-engine-tests")]
#![forbid(unsafe_code)]

use std::{env, path::PathBuf, time::Duration};

use kontainer_engine::{
    ContainerEngine, EngineAccessScope, EngineApiVersion, EngineErrorKind, LocalDockerEngine,
    LocalEngineOptions,
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
