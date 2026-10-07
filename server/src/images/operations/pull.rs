//! `image.pull`: pull one reference on every listed node in parallel, with per-node progress.
//!
//! One failing node never stops the others; its registry error is kept verbatim.

use std::{
    collections::BTreeMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use futures_util::{StreamExt, future::join_all};

use crate::{
    containers::operations::lifecycle::{action_failure, node_client},
    images::domain::{NodePull, NodePullStatus, PullImagePayload, PullTracker},
    operations::{
        domain::{DurableOperation, OperationKind},
        executor::{ExecutionContext, ExecutionFailure, OperationExecutor},
    },
    rpc::{AgentContainerAction, AgentPullEvent},
};

/// Progress is written at most this often (plus whenever a node finishes).
const REPORT_EVERY: Duration = Duration::from_millis(500);
/// A node that sends no pull event for this long fails, so one hung node doesn't hold the whole
/// pull to the operation's budget.
const IDLE_AT_MOST: Duration = Duration::from_secs(60);

struct PullBoard {
    nodes: Mutex<BTreeMap<String, NodePull>>,
    last_report: Mutex<Option<Instant>>,
}

impl PullBoard {
    fn detail(&self) -> serde_json::Value {
        let nodes: Vec<NodePull> = self
            .nodes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .cloned()
            .collect();
        serde_json::json!({ "nodes": nodes })
    }

    fn update(&self, pull: NodePull) {
        self.nodes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(pull.node_id.clone(), pull);
    }

    /// Whether this update should be written now.
    fn due(&self, force: bool) -> bool {
        let mut last = self
            .last_report
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        if force || last.is_none_or(|at| now.duration_since(at) >= REPORT_EVERY) {
            *last = Some(now);
            true
        } else {
            false
        }
    }
}

pub struct PullImageExecutor {
    idle: Duration,
}

impl Default for PullImageExecutor {
    fn default() -> Self {
        Self { idle: IDLE_AT_MOST }
    }
}

impl PullImageExecutor {
    /// The same executor giving up on a silent node sooner, for tests.
    #[must_use]
    pub const fn with_idle(idle: Duration) -> Self {
        Self { idle }
    }
}

#[async_trait]
impl OperationExecutor for PullImageExecutor {
    fn kind(&self) -> OperationKind {
        OperationKind::ImagePull
    }

    async fn execute(
        &self,
        operation: &DurableOperation,
        context: &ExecutionContext,
    ) -> Result<Option<serde_json::Value>, ExecutionFailure> {
        let payload: PullImagePayload = serde_json::from_value(operation.payload.clone())
            .map_err(|_| ExecutionFailure::invalid_payload())?;
        let board = PullBoard {
            nodes: Mutex::new(
                payload
                    .node_ids
                    .iter()
                    .map(|node_id| {
                        (
                            node_id.clone(),
                            PullTracker::default().snapshot(node_id, NodePullStatus::Pulling, None),
                        )
                    })
                    .collect(),
            ),
            last_report: Mutex::new(None),
        };
        context.report_progress(board.detail()).await;
        let board = &board;
        let reference = payload.reference.as_str();
        let idle = self.idle;
        join_all(payload.node_ids.iter().map(|node_id| async move {
            let finished = pull_one(node_id, reference, idle, context, board).await;
            board.update(finished);
            if board.due(true) {
                context.report_progress(board.detail()).await;
            }
        }))
        .await;

        let detail = board.detail();
        let nodes = board
            .nodes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let failed = nodes
            .values()
            .filter(|node| node.status == NodePullStatus::Failed)
            .count();
        if failed == 0 {
            return Ok(Some(detail));
        }
        let code = if failed == nodes.len() {
            "pull_failed"
        } else {
            "partial_failure"
        };
        Err(ExecutionFailure::new(
            code,
            format!("The pull failed on {failed} of {} nodes.", nodes.len()),
        )
        .with_detail(detail))
    }
}

/// Run one node's pull to its end and return its final state. A node silent for `idle` fails.
async fn pull_one(
    node_id: &str,
    reference: &str,
    idle: Duration,
    context: &ExecutionContext,
    board: &PullBoard,
) -> NodePull {
    let mut tracker = PullTracker::default();
    let failed = |tracker: &PullTracker, error: String| {
        tracker.snapshot(node_id, NodePullStatus::Failed, Some(error))
    };
    let client = match node_client(&context.database, context.agents.as_ref(), node_id).await {
        Ok(client) => client,
        Err(failure) => return failed(&tracker, failure.message),
    };
    let mut events = match client.pull_image(reference, &context.cancellation).await {
        Ok(events) => events,
        Err(error) => {
            return failed(
                &tracker,
                action_failure(error, AgentContainerAction::Start).message,
            );
        }
    };
    loop {
        let next = tokio::select! {
            biased;
            () = context.cancellation.cancelled() => {
                return failed(&tracker, "The pull was interrupted.".to_owned());
            }
            next = tokio::time::timeout(idle, events.next()) => match next {
                Ok(next) => next,
                Err(_) => {
                    let seconds = idle.as_secs();
                    return failed(&tracker, format!("No progress from the node for {seconds} s."));
                }
            },
        };
        match next {
            Some(AgentPullEvent::Progress {
                layer_id,
                status,
                current,
                total,
            }) => {
                tracker.record(layer_id.as_deref(), &status, current, total);
                board.update(tracker.snapshot(node_id, NodePullStatus::Pulling, None));
                if board.due(false) {
                    context.report_progress(board.detail()).await;
                }
            }
            Some(AgentPullEvent::Failed(message)) => return failed(&tracker, message),
            None => return tracker.snapshot(node_id, NodePullStatus::Done, None),
        }
    }
}
