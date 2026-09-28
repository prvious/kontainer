//! Bulk execution over many `(node, container)` targets with per-target results.

use std::future::Future;

use futures_util::{StreamExt, stream};
use serde::{Deserialize, Serialize};

use super::executor::{ExecutionContext, ExecutionFailure};

pub const MAX_BULK_TARGETS: usize = 200;
pub const BULK_CONCURRENCY: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BulkTarget {
    pub node_id: String,
    pub container_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkResult {
    pub node_id: String,
    pub container_id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The progress and result document of a bulk operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkDetail {
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub results: Vec<BulkResult>,
}

/// Run `act` over every target with concurrency 8, reporting the detail after each one. The
/// operation succeeds only when every target did; otherwise it fails with `partial_failure` and
/// the full results. `act` returns a short error code on failure.
pub async fn run_bulk<F, Fut>(
    targets: Vec<BulkTarget>,
    context: &ExecutionContext,
    act: F,
) -> Result<Option<serde_json::Value>, ExecutionFailure>
where
    F: Fn(BulkTarget) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let total = targets.len();
    let act = &act;
    let mut detail = BulkDetail {
        total,
        done: 0,
        failed: 0,
        results: Vec::with_capacity(total),
    };
    let mut outcomes = stream::iter(targets)
        .map(|target| async move {
            let result = act(target.clone()).await;
            (target, result)
        })
        .buffer_unordered(BULK_CONCURRENCY);
    while let Some((target, result)) = outcomes.next().await {
        detail.done += 1;
        let error = result.err();
        if error.is_some() {
            detail.failed += 1;
        }
        detail.results.push(BulkResult {
            node_id: target.node_id,
            container_id: target.container_id,
            ok: error.is_none(),
            error,
        });
        context.report_progress(to_value(&detail)).await;
    }
    if detail.failed == 0 {
        Ok(Some(to_value(&detail)))
    } else {
        Err(ExecutionFailure::new(
            "partial_failure",
            format!("{} of {} targets failed.", detail.failed, detail.total),
        )
        .with_detail(to_value(&detail)))
    }
}

fn to_value(detail: &BulkDetail) -> serde_json::Value {
    serde_json::to_value(detail).unwrap_or(serde_json::Value::Null)
}
