//! `noop`: exercises the operation pipeline end to end (development and test routes only).

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::operations::{
    domain::{DurableOperation, OperationKind},
    executor::{ExecutionContext, ExecutionFailure, OperationExecutor},
};

/// `{ steps, stepMs, failWith? }`: report `steps` progress documents `stepMs` apart, then succeed
/// or fail with the given code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NoopRequest {
    #[serde(default = "default_steps")]
    pub steps: u32,
    #[serde(default = "default_step_ms")]
    pub step_ms: u64,
    #[serde(default)]
    pub fail_with: Option<String>,
}

const fn default_steps() -> u32 {
    3
}

const fn default_step_ms() -> u64 {
    200
}

impl NoopRequest {
    /// Bounds checked at the HTTP edge before admission.
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(1..=20).contains(&self.steps) {
            return Err("steps must be between 1 and 20");
        }
        if self.step_ms > 5_000 {
            return Err("stepMs must be at most 5000");
        }
        if let Some(code) = &self.fail_with
            && (code.is_empty()
                || code.len() > 64
                || !code
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'_'))
        {
            return Err("failWith must be a snake_case code");
        }
        Ok(())
    }
}

pub struct NoopExecutor;

#[async_trait]
impl OperationExecutor for NoopExecutor {
    fn kind(&self) -> OperationKind {
        OperationKind::Noop
    }

    async fn execute(
        &self,
        operation: &DurableOperation,
        context: &ExecutionContext,
    ) -> Result<Option<serde_json::Value>, ExecutionFailure> {
        let request: NoopRequest = serde_json::from_value(operation.payload.clone())
            .map_err(|_| ExecutionFailure::invalid_payload())?;
        for done in 1..=request.steps {
            tokio::select! {
                () = context.cancellation.cancelled() => {
                    return Err(ExecutionFailure::new("cancelled", "The operation was interrupted."));
                }
                () = tokio::time::sleep(Duration::from_millis(request.step_ms)) => {}
            }
            context
                .report_progress(serde_json::json!({ "total": request.steps, "done": done }))
                .await;
        }
        match request.fail_with {
            Some(code) => Err(ExecutionFailure::new(
                code,
                "The test operation failed as requested.",
            )),
            None => Ok(Some(
                serde_json::json!({ "total": request.steps, "done": request.steps }),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NoopRequest;

    #[test]
    fn requests_are_bounded() {
        let parse = |value| serde_json::from_value::<NoopRequest>(value).unwrap();
        assert!(parse(serde_json::json!({})).validate().is_ok());
        assert_eq!(parse(serde_json::json!({})).steps, 3);
        assert!(parse(serde_json::json!({"steps": 0})).validate().is_err());
        assert!(parse(serde_json::json!({"steps": 21})).validate().is_err());
        assert!(
            parse(serde_json::json!({"stepMs": 5001}))
                .validate()
                .is_err()
        );
        assert!(
            parse(serde_json::json!({"failWith": "Bad Code"}))
                .validate()
                .is_err()
        );
        assert!(
            parse(serde_json::json!({"failWith": "no_eligible_nodes"}))
                .validate()
                .is_ok()
        );
        assert!(serde_json::from_value::<NoopRequest>(serde_json::json!({"extra": 1})).is_err());
    }
}
