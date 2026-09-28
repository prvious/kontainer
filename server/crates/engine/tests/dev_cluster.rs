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

/// Creates, lists, sizes, and removes a throwaway volume on worker-2, including Docker's refusal
/// while a (never started) container mounts it.
#[tokio::test]
async fn dev_cluster_volume_lifecycle_with_sizes_and_in_use_refusal() {
    use std::collections::BTreeMap;

    use bollard::{
        models::{ContainerCreateBody, HostConfig},
        query_parameters::{CreateContainerOptionsBuilder, RemoveContainerOptionsBuilder},
    };
    use kontainer_engine::{EngineErrorKind, VolumeCreate};

    let host = dev_host("KONTAINER_TEST_DEV_WORKER_2_HOST", "tcp://localhost:2378");
    let worker = engine(host.clone());
    let cancellation = CancellationToken::new();
    let name = format!("kontainer-engine-test-{}", std::process::id());
    let create = VolumeCreate {
        name: name.clone(),
        driver: "local".to_owned(),
        labels: BTreeMap::from([("purpose".to_owned(), "engine-test".to_owned())]),
        driver_opts: BTreeMap::new(),
    };
    let created = worker.create_volume(&create, &cancellation).await.unwrap();
    assert_eq!(
        (created.name.as_str(), created.driver.as_str()),
        (name.as_str(), "local")
    );
    assert_eq!(
        worker
            .create_volume(&create, &cancellation)
            .await
            .unwrap_err()
            .kind(),
        EngineErrorKind::Conflict,
        "an existing name is refused, not adopted"
    );

    let listed = worker.list_volumes(&cancellation).await.unwrap();
    let volume = listed.iter().find(|volume| volume.name == name).unwrap();
    assert_eq!(
        volume.labels.get("purpose").map(String::as_str),
        Some("engine-test")
    );
    assert_eq!(volume.used_by, 0);
    assert!(volume.created_at_seconds.is_some());
    let usage = worker.disk_usage(&cancellation).await.unwrap();
    assert_eq!(usage.volume_sizes.get(&name), Some(&0));
    assert!(usage.total_bytes.is_some());

    let address = host.to_string().replace("tcp://", "http://");
    let docker =
        bollard::Docker::connect_with_http(&address, 10, bollard::API_DEFAULT_VERSION).unwrap();
    let container_name = format!("{name}-user");
    // Any local image: the containerd image store resolves ids, not always bare tags.
    let image = worker.list_images(&cancellation).await.unwrap()[0]
        .image_id
        .clone();
    docker
        .create_container(
            Some(
                CreateContainerOptionsBuilder::new()
                    .name(&container_name)
                    .build(),
            ),
            ContainerCreateBody {
                image: Some(image),
                cmd: Some(vec!["true".to_owned()]),
                host_config: Some(HostConfig {
                    binds: Some(vec![format!("{name}:/data")]),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let mounted = worker.list_volumes(&cancellation).await.unwrap();
    let volume = mounted.iter().find(|volume| volume.name == name).unwrap();
    let users = (volume.used_by, volume.users.clone());
    let refused = worker.remove_volume(&name, &cancellation).await;
    docker
        .remove_container(
            &container_name,
            Some(RemoveContainerOptionsBuilder::new().force(true).build()),
        )
        .await
        .unwrap();
    assert_eq!(users, (1, vec![container_name.clone()]));
    assert_eq!(refused.unwrap_err().kind(), EngineErrorKind::Conflict);

    worker.remove_volume(&name, &cancellation).await.unwrap();
    assert_eq!(
        worker
            .remove_volume(&name, &cancellation)
            .await
            .unwrap_err()
            .kind(),
        EngineErrorKind::NotFound
    );
}
