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

/// Overlay networks are created on the manager with swarm scope; bridges stay on their node;
/// built-ins are refused before Docker is asked.
#[tokio::test]
async fn dev_cluster_network_lifecycle_on_the_manager_and_a_worker() {
    use kontainer_engine::{EngineErrorKind, NetworkCreate};

    let manager = engine(dev_host(
        "KONTAINER_TEST_DEV_MANAGER_HOST",
        "tcp://localhost:2375",
    ));
    let worker = engine(dev_host(
        "KONTAINER_TEST_DEV_WORKER_2_HOST",
        "tcp://localhost:2378",
    ));
    let cancellation = CancellationToken::new();
    let suffix = std::process::id();

    let overlay = NetworkCreate {
        name: format!("kontainer-test-overlay-{suffix}"),
        driver: "overlay".to_owned(),
        attachable: true,
        subnet: Some("10.231.0.0/24".to_owned()),
        ..Default::default()
    };
    let overlay_id = manager
        .create_network(&overlay, &cancellation)
        .await
        .unwrap();
    let listed = manager.list_networks(&cancellation).await.unwrap();
    let found = listed
        .iter()
        .find(|network| network.id == overlay_id)
        .unwrap();
    assert_eq!(
        (
            found.scope.as_str(),
            found.driver.as_str(),
            found.attachable
        ),
        ("swarm", "overlay", true)
    );
    assert_eq!(found.subnets, vec!["10.231.0.0/24"]);
    let ingress = listed
        .iter()
        .find(|network| network.name == "ingress")
        .unwrap();
    assert!(ingress.builtin() && ingress.ingress);
    assert_eq!(
        manager
            .remove_network(&ingress.id, &cancellation)
            .await
            .unwrap_err()
            .kind(),
        EngineErrorKind::InvalidInput
    );
    manager
        .remove_network(&overlay_id, &cancellation)
        .await
        .unwrap();

    let bridge = NetworkCreate {
        name: format!("kontainer-test-bridge-{suffix}"),
        driver: "bridge".to_owned(),
        ..Default::default()
    };
    let bridge_id = worker.create_network(&bridge, &cancellation).await.unwrap();
    let detail = worker
        .inspect_network(&bridge_id, &cancellation)
        .await
        .unwrap();
    assert_eq!(
        (detail.network.scope.as_str(), detail.endpoints.len()),
        ("local", 0)
    );
    worker
        .remove_network(&bridge_id, &cancellation)
        .await
        .unwrap();
    assert_eq!(
        worker
            .inspect_network(&bridge_id, &cancellation)
            .await
            .unwrap_err()
            .kind(),
        EngineErrorKind::NotFound
    );

    // A seed stack network with running tasks lists its endpoints on the node that runs them.
    let networks = worker.list_networks(&cancellation).await.unwrap();
    let seed = networks
        .iter()
        .find(|network| network.name == "toolbox_kontainer")
        .unwrap();
    assert!(seed.containers > 0);
    let seed_detail = worker
        .inspect_network(&seed.id, &cancellation)
        .await
        .unwrap();
    assert!(!seed_detail.endpoints.is_empty());
}

/// Services and tasks on the manager: list with modes and ports, versioned scaling (a stale
/// version is refused), force update, and Swarm status. Scales a seed service up and back.
#[tokio::test]
async fn dev_cluster_services_scale_with_versions_on_the_manager() {
    use kontainer_engine::{EngineErrorKind, ServiceMode};

    let manager = engine(dev_host(
        "KONTAINER_TEST_DEV_MANAGER_HOST",
        "tcp://localhost:2375",
    ));
    let cancellation = CancellationToken::new();
    let status = manager.swarm_status(&cancellation).await.unwrap();
    assert!(status.control_available);
    assert_eq!(status.nodes, 3);

    let services = manager.list_services(&cancellation).await.unwrap();
    let collector = services
        .iter()
        .find(|service| service.name == "monitoring_collector")
        .unwrap();
    assert_eq!(collector.mode, ServiceMode::Global);
    let redis = services
        .iter()
        .find(|service| service.name == "toolbox_redis")
        .unwrap();
    assert_eq!(redis.volumes, vec!["toolbox_redis-data".to_owned()]);
    assert_eq!(collector.stack.as_deref(), Some("monitoring"));
    let alpine = services
        .iter()
        .find(|service| service.name == "toolbox_alpine")
        .unwrap()
        .clone();
    let ServiceMode::Replicated { replicas } = alpine.mode else {
        panic!("toolbox_alpine is replicated");
    };
    assert!(alpine.image.starts_with("alpine:latest"));

    let tasks = manager
        .list_tasks(std::slice::from_ref(&alpine.id), true, &cancellation)
        .await
        .unwrap();
    assert_eq!(
        tasks.iter().filter(|task| task.is_running()).count() as u64,
        replicas
    );
    assert!(tasks.iter().all(|task| task.service_id == alpine.id));

    manager
        .scale_service(&alpine.id, replicas + 1, alpine.version, &cancellation)
        .await
        .unwrap();
    assert_eq!(
        manager
            .scale_service(&alpine.id, replicas, alpine.version, &cancellation)
            .await
            .unwrap_err()
            .kind(),
        EngineErrorKind::VersionConflict,
        "the version moved on"
    );
    let scaled = manager
        .list_services(&cancellation)
        .await
        .unwrap()
        .into_iter()
        .find(|service| service.id == alpine.id)
        .unwrap();
    assert_eq!(
        scaled.mode,
        ServiceMode::Replicated {
            replicas: replicas + 1
        }
    );
    assert!(scaled.version > alpine.version);
    manager
        .scale_service(&alpine.id, replicas, scaled.version, &cancellation)
        .await
        .unwrap();
}
