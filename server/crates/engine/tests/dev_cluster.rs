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
    assert!(ingress.builtin());
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

/// A TTY shell in a running alpine-based seed container on the manager: `echo ok` comes back,
/// `exit` reports 0, and a shell that doesn't exist ends with 126 or 127.
#[tokio::test]
async fn dev_cluster_exec_runs_a_tty_shell_and_reports_its_exit() {
    use futures_util::StreamExt as _;
    use kontainer_engine::{ContainerPageRequest, is_missing_shell_status};

    let manager = engine(dev_host(
        "KONTAINER_TEST_DEV_MANAGER_HOST",
        "tcp://localhost:2375",
    ));
    let cancellation = CancellationToken::new();
    let page = manager
        .list_containers(ContainerPageRequest::new(200, "").unwrap(), &cancellation)
        .await
        .unwrap();
    let alpine = page
        .containers
        .iter()
        .find(|container| {
            container.image_reference.contains("alpine")
                && container.state == kontainer_engine::ContainerState::Running
        })
        .expect("a running alpine-based seed container on the manager");
    let env = vec!["TERM=xterm-256color".to_owned()];

    let exec = manager
        .exec_create(
            &alpine.container_id,
            &["/bin/sh".to_owned()],
            &env,
            &cancellation,
        )
        .await
        .unwrap();
    let mut attached = manager.exec_start(&exec, &cancellation).await.unwrap();
    manager
        .exec_resize(&exec, 100, 30, &cancellation)
        .await
        .unwrap();
    attached.input.write(b"echo o''k\r").await.unwrap();
    let mut seen = Vec::new();
    let found = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Some(Ok(bytes)) = attached.output.next().await {
            seen.extend_from_slice(&bytes);
            if String::from_utf8_lossy(&seen).contains("ok\r\n") {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(found, "{:?}", String::from_utf8_lossy(&seen));
    attached.input.write(b"exit\r").await.unwrap();
    while attached.output.next().await.is_some() {}
    let mut status = manager.exec_inspect(&exec, &cancellation).await.unwrap();
    for _ in 0..20 {
        if !status.running {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        status = manager.exec_inspect(&exec, &cancellation).await.unwrap();
    }
    assert_eq!(status.exit_code, Some(0));

    let missing = manager
        .exec_create(
            &alpine.container_id,
            &["/no/such/shell".to_owned()],
            &env,
            &cancellation,
        )
        .await
        .unwrap();
    let mut attached = manager.exec_start(&missing, &cancellation).await.unwrap();
    while attached.output.next().await.is_some() {}
    let mut status = manager.exec_inspect(&missing, &cancellation).await.unwrap();
    for _ in 0..20 {
        if !status.running {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        status = manager.exec_inspect(&missing, &cancellation).await.unwrap();
    }
    assert!(is_missing_shell_status(status.exit_code), "{status:?}");
}

/// Closing a TTY shell's input ends its output but not the shell: Docker closes the attach
/// without passing end of file to the terminal, so the shell is still running afterwards and
/// an agent can't report that close as an exit.
#[tokio::test]
async fn dev_cluster_exec_shell_outlives_its_closed_input() {
    use futures_util::StreamExt as _;
    use kontainer_engine::ContainerPageRequest;

    let manager = engine(dev_host(
        "KONTAINER_TEST_DEV_MANAGER_HOST",
        "tcp://localhost:2375",
    ));
    let cancellation = CancellationToken::new();
    let page = manager
        .list_containers(ContainerPageRequest::new(200, "").unwrap(), &cancellation)
        .await
        .unwrap();
    let alpine = page
        .containers
        .iter()
        .find(|container| {
            container.image_reference.contains("alpine")
                && container.state == kontainer_engine::ContainerState::Running
        })
        .expect("a running alpine-based seed container on the manager");
    let env = vec!["TERM=xterm-256color".to_owned()];
    let exec = manager
        .exec_create(
            &alpine.container_id,
            &["/bin/sh".to_owned()],
            &env,
            &cancellation,
        )
        .await
        .unwrap();
    let mut attached = manager.exec_start(&exec, &cancellation).await.unwrap();
    // The shell's own pid, so the test can stop it afterwards.
    attached.input.write(b"echo pid=$$\r").await.unwrap();
    let mut seen = String::new();
    let pid = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(Ok(bytes)) = attached.output.next().await {
            seen.push_str(&String::from_utf8_lossy(&bytes));
            let pid = seen.split("pid=").skip(1).find_map(|rest| {
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                (!digits.is_empty() && rest[digits.len()..].starts_with('\r')).then_some(digits)
            });
            if pid.is_some() {
                return pid;
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
    .expect("the shell prints its pid");

    attached.input.close().await;
    let drained = tokio::time::timeout(Duration::from_secs(5), async {
        while attached.output.next().await.is_some() {}
    })
    .await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let status = manager.exec_inspect(&exec, &cancellation).await.unwrap();

    let kill = manager
        .exec_create(
            &alpine.container_id,
            &["kill".to_owned(), "-9".to_owned(), pid],
            &env,
            &cancellation,
        )
        .await
        .unwrap();
    let mut killing = manager.exec_start(&kill, &cancellation).await.unwrap();
    while killing.output.next().await.is_some() {}

    assert!(drained.is_ok(), "the output ends once the input closes");
    assert!(status.running, "{status:?}");
}

/// A pull that can't succeed ends after one failure that carries the daemon's own message.
#[tokio::test]
async fn dev_cluster_a_failed_pull_ends_with_one_failure() {
    use futures_util::StreamExt as _;
    use kontainer_engine::PullEvent;

    let manager = engine(dev_host(
        "KONTAINER_TEST_DEV_MANAGER_HOST",
        "tcp://localhost:2375",
    ));
    let events: Vec<_> = tokio::time::timeout(
        Duration::from_secs(60),
        manager
            .pull_image(
                "kontainer.invalid/no-such-image:missing",
                &CancellationToken::new(),
            )
            .await
            .unwrap()
            .collect(),
    )
    .await
    .expect("the pull ends");
    let failures = events
        .iter()
        .filter(|event| matches!(event, PullEvent::Failed(_)))
        .count();
    assert_eq!(failures, 1, "{events:?}");
    assert!(
        matches!(events.last(), Some(PullEvent::Failed(message)) if !message.is_empty()),
        "{events:?}"
    );
}

/// A seed service's logs through the manager: history with task and node ids, no follow.
#[tokio::test]
async fn dev_cluster_service_logs_carry_task_and_node_ids() {
    use futures_util::StreamExt as _;
    use kontainer_engine::ServiceLogRequest;

    let manager = engine(dev_host(
        "KONTAINER_TEST_DEV_MANAGER_HOST",
        "tcp://localhost:2375",
    ));
    let cancellation = CancellationToken::new();
    let services = manager.list_services(&cancellation).await.unwrap();
    let web = services
        .iter()
        .find(|service| service.name == "webapp_web")
        .unwrap();
    let request = ServiceLogRequest {
        since_unix_ms: None,
        tail: 5,
        follow: false,
    };
    let lines: Vec<_> = manager
        .service_logs(&web.id, request, &cancellation)
        .await
        .unwrap()
        .collect()
        .await;
    assert!(!lines.is_empty(), "whoami logs its start-up");
    for line in lines {
        let line = line.unwrap();
        assert_eq!(line.node_id.len(), 25, "{line:?}");
        assert_eq!(line.task_id.len(), 25, "{line:?}");
        assert!(line.timestamp_unix_ms > 1_700_000_000_000);
    }
    assert!(
        manager
            .service_logs("not-a-service", request, &cancellation)
            .await
            .is_err()
    );
    // A well-formed id of no service: the call succeeds and Docker's 404 is the only item.
    let unknown: Vec<_> = manager
        .service_logs("zzzzzzzzzzzzzzzzzzzzzzzzz", request, &cancellation)
        .await
        .unwrap()
        .collect()
        .await;
    assert!(
        matches!(
            unknown.as_slice(),
            [Err(error)] if error.kind() == kontainer_engine::EngineErrorKind::NotFound
        ),
        "{unknown:?}"
    );
}

/// Swarm members on the manager: one leader manager and two ready workers with resources.
#[tokio::test]
async fn dev_cluster_swarm_nodes_have_one_leader_and_two_workers() {
    use kontainer_engine::{SwarmNodeState, SwarmRole};

    let manager = engine(dev_host(
        "KONTAINER_TEST_DEV_MANAGER_HOST",
        "tcp://localhost:2375",
    ));
    let cancellation = CancellationToken::new();
    let nodes = manager.list_swarm_nodes(&cancellation).await.unwrap();
    assert_eq!(nodes.len(), 3);
    let leaders = nodes
        .iter()
        .filter(|node| node.is_leader)
        .collect::<Vec<_>>();
    assert_eq!(leaders.len(), 1);
    assert_eq!(leaders[0].role, SwarmRole::Manager);
    assert_eq!(leaders[0].manager_reachable, Some(true));
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node.role == SwarmRole::Worker)
            .count(),
        2
    );
    for node in &nodes {
        assert_eq!(node.state, SwarmNodeState::Ready, "{}", node.hostname);
        assert!(node.cpu_cores.is_some_and(|cores| cores > 0));
        assert!(node.address.is_some());
    }
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
    manager.swarm_status(&cancellation).await.unwrap();

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
