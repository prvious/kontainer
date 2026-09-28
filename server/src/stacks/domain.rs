//! Stacks: services grouped by `com.docker.stack.namespace`, with container figures summed from
//! the stack's containers, and what removing one takes away.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{containers::domain::ContainerItem, services::domain::ServiceRow};

/// Docker's label for the stack a service, network, or volume belongs to.
pub const STACK_LABEL: &str = "com.docker.stack.namespace";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackStatus {
    /// Every service converged.
    Running,
    Partial,
    /// No task runs.
    Stopped,
}

impl StackStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Partial => "partial",
            Self::Stopped => "stopped",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StackRow {
    pub name: String,
    pub services: u32,
    /// Tasks running and desired, summed over the stack's services.
    pub running: u64,
    pub desired: u64,
    /// Summed from the stack's running containers; `None` when no figure was sampled.
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<u64>,
    /// The newest service update.
    pub last_deployed_seconds: Option<i64>,
    pub status: StackStatus,
}

/// Group services by stack and sum the stack's container figures. Services without the stack
/// label are not part of any stack.
#[must_use]
pub fn group_stacks(services: &[ServiceRow], containers: &[ContainerItem]) -> Vec<StackRow> {
    let mut stacks: BTreeMap<&str, StackRow> = BTreeMap::new();
    for row in services {
        let Some(name) = row.service.stack.as_deref() else {
            continue;
        };
        let stack = stacks.entry(name).or_insert_with(|| StackRow {
            name: name.to_owned(),
            services: 0,
            running: 0,
            desired: 0,
            cpu_percent: None,
            memory_bytes: None,
            last_deployed_seconds: None,
            status: StackStatus::Running,
        });
        stack.services += 1;
        stack.running += row.running;
        stack.desired += row.desired;
        stack.last_deployed_seconds = stack
            .last_deployed_seconds
            .max(row.service.updated_at_seconds);
        if row.running < row.desired {
            stack.status = StackStatus::Partial;
        }
    }
    for container in containers {
        let Some(stack) = container
            .stack_namespace
            .as_deref()
            .and_then(|name| stacks.get_mut(name))
        else {
            continue;
        };
        if let Some(cpu) = container.cpu_percent {
            stack.cpu_percent = Some(stack.cpu_percent.unwrap_or(0.0) + cpu);
        }
        if let Some(memory) = container.memory_used_bytes {
            stack.memory_bytes = Some(stack.memory_bytes.unwrap_or(0).saturating_add(memory));
        }
    }
    let mut rows: Vec<StackRow> = stacks.into_values().collect();
    for row in &mut rows {
        if row.running == 0 {
            row.status = StackStatus::Stopped;
        }
    }
    rows
}

/// What removing a stack takes away, and what it leaves.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovalPlan {
    pub services: Vec<String>,
    /// Networks carrying the stack label.
    pub networks: Vec<String>,
    /// Named volumes the services mount; removal never touches them.
    pub volumes_retained: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackRemovePayload {
    pub name: String,
}

/// Stack names as Docker allows them.
#[must_use]
pub fn is_valid_stack_name(value: &str) -> bool {
    kontainer_engine::is_valid_network_name(value)
}

#[cfg(test)]
mod tests {
    use kontainer_engine::{ServiceMode, ServiceSummary};

    use super::{StackStatus, group_stacks};
    use crate::{
        containers::domain::ContainerItem, rpc::AgentContainerState, services::domain::ServiceRow,
    };

    fn service(
        name: &str,
        stack: Option<&str>,
        running: u64,
        desired: u64,
        updated: i64,
    ) -> ServiceRow {
        ServiceRow {
            service: ServiceSummary {
                id: format!("{name:0<25}"),
                name: name.to_owned(),
                mode: ServiceMode::Replicated { replicas: desired },
                stack: stack.map(str::to_owned),
                updated_at_seconds: Some(updated),
                ..ServiceSummary::default()
            },
            running,
            desired,
        }
    }

    fn container(stack: &str, cpu: Option<f64>, memory: Option<u64>) -> ContainerItem {
        ContainerItem {
            node_id: "manager".to_owned(),
            node_name: "manager".to_owned(),
            container_id: "a".repeat(64),
            name: format!("{stack}_x.1"),
            image: "nginx".to_owned(),
            state: AgentContainerState::Running,
            status: "Up".to_owned(),
            created_at: chrono::DateTime::UNIX_EPOCH,
            started_at: None,
            finished_at: None,
            ports: Vec::new(),
            service_name: None,
            stack_namespace: Some(stack.to_owned()),
            task_id: None,
            cpu_percent: cpu,
            memory_used_bytes: memory,
            memory_limit_bytes: None,
            networks: Vec::new(),
        }
    }

    #[test]
    fn services_group_by_stack_with_status_and_figures() {
        let rows = group_stacks(
            &[
                service("webapp_web", Some("webapp"), 3, 3, 100),
                service("webapp_api", Some("webapp"), 1, 2, 300),
                service("toolbox_nginx", Some("toolbox"), 2, 2, 50),
                service("idle_worker", Some("idle"), 0, 1, 10),
                service("loner", None, 1, 1, 10),
            ],
            &[
                container("webapp", Some(12.5), Some(100)),
                container("webapp", Some(2.5), None),
                container("toolbox", None, Some(64)),
            ],
        );
        let summary: Vec<_> = rows
            .iter()
            .map(|row| {
                (
                    row.name.as_str(),
                    row.services,
                    row.running,
                    row.desired,
                    row.status,
                    row.last_deployed_seconds,
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                ("idle", 1, 0, 1, StackStatus::Stopped, Some(10)),
                ("toolbox", 1, 2, 2, StackStatus::Running, Some(50)),
                ("webapp", 2, 4, 5, StackStatus::Partial, Some(300)),
            ]
        );
        assert_eq!(
            (rows[2].cpu_percent, rows[2].memory_bytes),
            (Some(15.0), Some(100))
        );
        assert_eq!(
            (rows[1].cpu_percent, rows[1].memory_bytes),
            (None, Some(64))
        );
        assert_eq!((rows[0].cpu_percent, rows[0].memory_bytes), (None, None));
    }
}
