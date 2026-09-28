#![cfg(feature = "real-engine-tests")]
#![forbid(unsafe_code)]

//! Runs against the docker-in-docker Swarm from `dev/docker-compose.yml`
//! (`pnpm dev:cluster`). Hosts default to the published dev ports and can be
//! overridden with `KONTAINER_TEST_DEV_MANAGER_HOST` / `..._WORKER_HOST`.

use std::{env, time::Duration};

use kontainer_engine::{
    CapabilityStatus, ContainerEngine, DockerHost, LocalDockerEngine, LocalEngineOptions,
    UnsupportedCapabilityReason,
};
use tokio_util::sync::CancellationToken;

fn dev_host(variable: &str, default: &str) -> DockerHost {
    env::var(variable)
        .unwrap_or_else(|_| default.to_owned())
        .parse()
        .expect("dev cluster host must be tcp://host:port")
}

fn engine(host: DockerHost) -> LocalDockerEngine {
    LocalDockerEngine::connect_for_node(LocalEngineOptions {
        host,
        operation_timeout: Duration::from_secs(10),
    })
    .unwrap()
}

#[tokio::test]
async fn dev_cluster_manager_reports_swarm_manager_over_tcp() {
    let manager = engine(dev_host(
        "KONTAINER_TEST_DEV_MANAGER_HOST",
        "tcp://localhost:2375",
    ));
    let description = manager.describe(&CancellationToken::new()).await.unwrap();
    assert_eq!(
        description.capabilities.swarm_manager_operations,
        CapabilityStatus::Supported
    );
}

#[tokio::test]
async fn dev_cluster_worker_reports_swarm_worker_over_tcp() {
    let worker = engine(dev_host(
        "KONTAINER_TEST_DEV_WORKER_HOST",
        "tcp://localhost:2376",
    ));
    let description = worker.describe(&CancellationToken::new()).await.unwrap();
    assert_eq!(
        description.capabilities.swarm_node_operations,
        CapabilityStatus::Supported
    );
    assert_eq!(
        description.capabilities.swarm_manager_operations,
        CapabilityStatus::Unsupported(UnsupportedCapabilityReason::ManagerControlUnavailable)
    );
}
