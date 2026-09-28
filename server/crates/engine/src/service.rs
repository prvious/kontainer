//! Swarm services, tasks, and nodes in Kontainer's own terms, read and changed on a manager.

/// How a service schedules its tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceMode {
    /// A fixed number of replicas.
    Replicated { replicas: u64 },
    /// One task per eligible node.
    Global,
}

impl Default for ServiceMode {
    fn default() -> Self {
        Self::Replicated { replicas: 0 }
    }
}

/// One service as the manager sees it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServiceSummary {
    pub id: String,
    pub name: String,
    /// The full reference the tasks run, digest included.
    pub image: String,
    pub mode: ServiceMode,
    /// Published ports, `published:target/proto`.
    pub ports: Vec<String>,
    pub created_at_seconds: Option<i64>,
    pub updated_at_seconds: Option<i64>,
    /// Swarm's object version; every write is based on it.
    pub version: u64,
    /// `com.docker.stack.namespace`.
    pub stack: Option<String>,
    /// `updating`, `paused`, `completed`, `rollback_started`, … when an update ran.
    pub update_state: Option<String>,
    pub update_message: Option<String>,
    /// Named volumes its tasks mount (`type: volume` sources).
    pub volumes: Vec<String>,
}

/// One task of a service.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskSummary {
    pub id: String,
    pub service_id: String,
    /// Replicated services number their tasks; global ones don't.
    pub slot: Option<u64>,
    pub node_id: Option<String>,
    /// `new`, `pending`, `assigned`, …, `running`, `complete`, `shutdown`, `failed`, `rejected`.
    pub state: String,
    pub desired_state: String,
    /// The task's error, e.g. "no suitable node (insufficient memory on 3 nodes)".
    pub error: Option<String>,
    pub message: Option<String>,
    pub updated_at_seconds: Option<i64>,
    pub container_id: Option<String>,
}

impl TaskSummary {
    /// Running and meant to be: what counts toward a service's running replicas.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.state == "running" && self.desired_state == "running"
    }
}

/// Whether this manager can answer cluster questions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SwarmStatus {
    pub control_available: bool,
    pub managers: u32,
    pub nodes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SwarmRole {
    Manager,
    #[default]
    Worker,
}

/// Whether the scheduler may place tasks on a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwarmAvailability {
    Active,
    Pause,
    Drain,
}

/// What the manager last heard from a node; `disconnected` counts as down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SwarmNodeState {
    Ready,
    Down,
    #[default]
    Unknown,
}

/// One Swarm member as the manager sees it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SwarmNode {
    pub id: String,
    pub hostname: String,
    pub role: SwarmRole,
    pub is_leader: bool,
    /// Raft reachability; `None` for workers.
    pub manager_reachable: Option<bool>,
    pub availability: Option<SwarmAvailability>,
    pub state: SwarmNodeState,
    /// The address the manager sees the node at.
    pub address: Option<String>,
    pub docker_version: Option<String>,
    pub os: Option<String>,
    pub architecture: Option<String>,
    pub cpu_cores: Option<u32>,
    pub memory_total_bytes: Option<u64>,
}

/// Swarm ids: 25 lowercase alphanumerics.
#[must_use]
pub fn is_valid_swarm_id(value: &str) -> bool {
    value.len() == 25
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase())
}

/// A reference without its `@sha256:` digest, as the list shows it.
#[must_use]
pub fn image_without_digest(reference: &str) -> &str {
    reference.split('@').next().unwrap_or(reference)
}

#[cfg(test)]
mod tests {
    use super::{TaskSummary, image_without_digest, is_valid_swarm_id};

    #[test]
    fn ids_images_and_running_tasks() {
        assert!(is_valid_swarm_id("x7kq2m9p0r1s3t5u7w9y0z2b4"));
        assert!(!is_valid_swarm_id("X7KQ2M9P0R1S3T5U7W9Y0Z2B4"));
        assert!(!is_valid_swarm_id("short"));
        assert_eq!(
            image_without_digest(&format!("nginx:alpine@sha256:{}", "a".repeat(64))),
            "nginx:alpine"
        );
        assert_eq!(image_without_digest("nginx:alpine"), "nginx:alpine");
        let task = |state: &str, desired: &str| TaskSummary {
            state: state.to_owned(),
            desired_state: desired.to_owned(),
            ..TaskSummary::default()
        };
        assert!(task("running", "running").is_running());
        assert!(!task("running", "shutdown").is_running(), "being replaced");
        assert!(!task("pending", "running").is_running());
    }
}
