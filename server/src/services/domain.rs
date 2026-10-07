//! Swarm services with running counts from their tasks, and the rules service operations wait
//! on: task counts for scaling, Swarm's update status for restart and rollback.

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Scaling has converged when the running count matches and no task Swarm is stopping still
/// runs. Swarm picks which slots go, so slot numbers say nothing.
#[must_use]
pub fn scale_converged(row: &ServiceRow, tasks: &[TaskSummary]) -> bool {
    row.running == row.desired
        && !tasks
            .iter()
            .any(|task| task.state == "running" && task.desired_state != "running")
}

/// Swarm's verdict on the update a restart or rollback started, from its update status (ADR
/// 0034): `None` while it runs or hasn't started, otherwise `Ok`, or the failure code with what
/// Swarm did.
#[must_use]
pub fn update_verdict(
    action: ServiceAction,
    row: &ServiceRow,
) -> Option<Result<(), (&'static str, &'static str)>> {
    match (action, row.service.update_state.as_deref()) {
        (_, Some("completed")) | (ServiceAction::Rollback, Some("rollback_completed")) => {
            Some(Ok(()))
        }
        (_, Some("paused")) => Some(Err(("update_paused", "paused the update"))),
        (_, Some("rollback_paused")) => Some(Err(("rollback_paused", "paused the rollback"))),
        // The service's failure action rolled the restart back.
        (_, Some("rollback_completed")) => Some(Err(("rolled_back", "rolled back the update"))),
        // With no task to replace, Swarm never starts the update.
        (ServiceAction::Restart, None) if row.desired == 0 => Some(Ok(())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use kontainer_engine::{ServiceMode, ServiceSummary, TaskSummary};

    use super::{
        ServiceAction, ServiceRow, latest_task_error, scale_converged, service_rows, update_verdict,
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

    fn row(replicas: u64, tasks: &[TaskSummary], update_state: Option<&str>) -> ServiceRow {
        let service = ServiceSummary {
            id: "web".to_owned(),
            mode: ServiceMode::Replicated { replicas },
            update_state: update_state.map(str::to_owned),
            ..ServiceSummary::default()
        };
        service_rows(vec![service], tasks).remove(0)
    }

    #[test]
    fn scaling_converges_on_counts_and_no_stopping_task_left_running() {
        let up = [
            task("web", Some(1), "running", "running"),
            task("web", Some(2), "starting", "running"),
        ];
        assert!(
            !scale_converged(&row(2, &up, None), &up),
            "accepted is not converged"
        );
        let down = [
            task("web", Some(1), "running", "running"),
            task("web", Some(2), "running", "shutdown"),
        ];
        assert!(
            !scale_converged(&row(1, &down, None), &down),
            "slot 2 still runs"
        );
        assert!(scale_converged(&row(1, &down[..1], None), &down[..1]));

        // Swarm removed slot 1 and kept slots 2 and 3.
        let stopping = [
            task("web", Some(1), "running", "remove"),
            task("web", Some(2), "running", "running"),
            task("web", Some(3), "running", "running"),
        ];
        assert!(!scale_converged(&row(2, &stopping, None), &stopping));
        let kept = [
            task("web", Some(1), "shutdown", "remove"),
            task("web", Some(2), "running", "running"),
            task("web", Some(3), "running", "running"),
        ];
        assert!(
            scale_converged(&row(2, &kept, None), &kept),
            "kept slots above the new count"
        );
    }

    #[test]
    fn restart_and_rollback_follow_swarms_update_status() {
        let tasks = [task("web", Some(1), "running", "running")];
        let verdict = |action, state| update_verdict(action, &row(1, &tasks, state));
        for action in [ServiceAction::Restart, ServiceAction::Rollback] {
            assert_eq!(verdict(action, None), None, "not started yet");
            assert_eq!(verdict(action, Some("updating")), None);
            assert_eq!(verdict(action, Some("rollback_started")), None);
            assert_eq!(verdict(action, Some("completed")), Some(Ok(())));
            assert_eq!(
                verdict(action, Some("paused")),
                Some(Err(("update_paused", "paused the update")))
            );
            assert_eq!(
                verdict(action, Some("rollback_paused")),
                Some(Err(("rollback_paused", "paused the rollback")))
            );
        }
        assert_eq!(
            verdict(ServiceAction::Rollback, Some("rollback_completed")),
            Some(Ok(()))
        );
        assert_eq!(
            verdict(ServiceAction::Restart, Some("rollback_completed")),
            Some(Err(("rolled_back", "rolled back the update"))),
            "the failure action undid the restart"
        );
        assert_eq!(
            update_verdict(ServiceAction::Restart, &row(0, &[], None)),
            Some(Ok(())),
            "nothing to replace"
        );
        assert_eq!(
            update_verdict(ServiceAction::Rollback, &row(0, &[], None)),
            None
        );
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
