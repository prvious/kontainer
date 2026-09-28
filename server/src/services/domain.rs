//! Swarm services with running counts from their tasks, and the convergence rules service
//! operations wait on.

use std::collections::HashMap;

use kontainer_engine::{ServiceMode, ServiceSummary, TaskSummary};
use serde::{Deserialize, Serialize};

/// One service with its replica counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceRow {
    pub service: ServiceSummary,
    /// Replicated: the replicas asked for. Global: the tasks meant to run (one per eligible node).
    pub desired: u64,
    /// Tasks running and meant to run.
    pub running: u64,
}

/// Count each service's tasks from one `tasks` call.
#[must_use]
pub fn service_rows(services: Vec<ServiceSummary>, tasks: &[TaskSummary]) -> Vec<ServiceRow> {
    let mut running: HashMap<&str, u64> = HashMap::new();
    let mut scheduled: HashMap<&str, u64> = HashMap::new();
    for task in tasks {
        if task.desired_state == "running" {
            *scheduled.entry(task.service_id.as_str()).or_default() += 1;
        }
        if task.is_running() {
            *running.entry(task.service_id.as_str()).or_default() += 1;
        }
    }
    let mut rows: Vec<ServiceRow> = services
        .into_iter()
        .map(|service| {
            let desired = match service.mode {
                ServiceMode::Replicated { replicas } => replicas,
                ServiceMode::Global => scheduled.get(service.id.as_str()).copied().unwrap_or(0),
            };
            let running = running.get(service.id.as_str()).copied().unwrap_or(0);
            ServiceRow {
                service,
                desired,
                running,
            }
        })
        .collect();
    rows.sort_by(|left, right| left.service.name.cmp(&right.service.name));
    rows
}

/// The newest task error, e.g. "no suitable node (insufficient memory on 3 nodes)".
#[must_use]
pub fn latest_task_error(tasks: &[TaskSummary]) -> Option<String> {
    tasks
        .iter()
        .filter(|task| task.error.is_some())
        .max_by_key(|task| task.updated_at_seconds.unwrap_or(0))
        .and_then(|task| task.error.clone())
}

/// What a service operation asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceAction {
    Scale,
    Restart,
    Rollback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceActionPayload {
    pub service_id: String,
    pub name: String,
    /// The service version the request was based on.
    pub version: u64,
    /// Scale only.
    pub replicas: Option<u64>,
}

/// Where a converging service stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Convergence {
    pub desired: u64,
    pub running: u64,
    pub converged: bool,
}

/// Scaling converges when the running count matches and, going down, no task above the new
/// count still runs.
#[must_use]
pub fn scale_convergence(tasks: &[TaskSummary], desired: u64) -> Convergence {
    let running = tasks.iter().filter(|task| task.is_running()).count() as u64;
    let lingering = tasks
        .iter()
        .any(|task| task.state == "running" && task.slot.is_some_and(|slot| slot > desired));
    Convergence {
        desired,
        running,
        converged: running == desired && !lingering,
    }
}

/// A restart converges when every running task started after it was asked for.
#[must_use]
pub fn restart_convergence(tasks: &[TaskSummary], desired: u64, since_seconds: i64) -> Convergence {
    let running: Vec<&TaskSummary> = tasks.iter().filter(|task| task.is_running()).collect();
    let fresh = running.iter().all(|task| {
        task.updated_at_seconds
            .is_some_and(|at| at >= since_seconds)
    });
    Convergence {
        desired,
        running: running.len() as u64,
        converged: running.len() as u64 == desired && fresh,
    }
}

/// A rollback converges when Swarm is no longer rolling back and the counts match.
#[must_use]
pub fn rollback_convergence(
    tasks: &[TaskSummary],
    desired: u64,
    update_state: Option<&str>,
) -> Convergence {
    let running = tasks.iter().filter(|task| task.is_running()).count() as u64;
    let busy = matches!(update_state, Some("updating" | "rollback_started"));
    Convergence {
        desired,
        running,
        converged: running == desired && !busy,
    }
}

#[cfg(test)]
mod tests {
    use kontainer_engine::{ServiceMode, ServiceSummary, TaskSummary};

    use super::{
        latest_task_error, restart_convergence, rollback_convergence, scale_convergence,
        service_rows,
    };

    fn task(service: &str, slot: Option<u64>, state: &str, desired: &str) -> TaskSummary {
        TaskSummary {
            id: format!("{service}{}", slot.unwrap_or(0)),
            service_id: service.to_owned(),
            slot,
            state: state.to_owned(),
            desired_state: desired.to_owned(),
            updated_at_seconds: Some(100),
            ..TaskSummary::default()
        }
    }

    #[test]
    fn running_counts_come_from_one_task_list() {
        let services = vec![
            ServiceSummary {
                id: "web".to_owned(),
                name: "shop_web".to_owned(),
                mode: ServiceMode::Replicated { replicas: 3 },
                ..ServiceSummary::default()
            },
            ServiceSummary {
                id: "agent".to_owned(),
                name: "monitoring_agent".to_owned(),
                mode: ServiceMode::Global,
                ..ServiceSummary::default()
            },
        ];
        let tasks = vec![
            task("web", Some(1), "running", "running"),
            task("web", Some(2), "running", "running"),
            task("web", Some(3), "pending", "running"),
            task("web", Some(4), "running", "shutdown"),
            task("agent", None, "running", "running"),
            task("agent", None, "running", "running"),
        ];
        let rows = service_rows(services, &tasks);
        let counts: Vec<_> = rows
            .iter()
            .map(|row| (row.service.name.as_str(), row.running, row.desired))
            .collect();
        assert_eq!(counts, vec![("monitoring_agent", 2, 2), ("shop_web", 2, 3)]);
    }

    #[test]
    fn convergence_rules() {
        let up = [
            task("web", Some(1), "running", "running"),
            task("web", Some(2), "starting", "running"),
        ];
        assert!(
            !scale_convergence(&up, 2).converged,
            "accepted is not converged"
        );
        let down = [
            task("web", Some(1), "running", "running"),
            task("web", Some(2), "running", "shutdown"),
        ];
        let state = scale_convergence(&down, 1);
        assert_eq!(
            (state.running, state.converged),
            (1, false),
            "slot 2 still runs"
        );
        assert!(scale_convergence(&down[..1], 1).converged);

        let restarted = [task("web", Some(1), "running", "running")];
        assert!(
            !restart_convergence(&restarted, 1, 200).converged,
            "the old task still runs"
        );
        assert!(restart_convergence(&restarted, 1, 50).converged);
        assert!(!rollback_convergence(&restarted, 1, Some("rollback_started")).converged);
        assert!(rollback_convergence(&restarted, 1, Some("rollback_completed")).converged);
    }

    #[test]
    fn the_newest_task_error_wins() {
        let mut old = task("web", Some(1), "rejected", "shutdown");
        old.error = Some("old".to_owned());
        old.updated_at_seconds = Some(10);
        let mut new = task("web", Some(2), "pending", "running");
        new.error = Some("no suitable node (insufficient memory on 3 nodes)".to_owned());
        new.updated_at_seconds = Some(20);
        assert_eq!(
            latest_task_error(&[old, new]).as_deref(),
            Some("no suitable node (insufficient memory on 3 nodes)")
        );
        assert_eq!(latest_task_error(&[]), None);
    }
}
