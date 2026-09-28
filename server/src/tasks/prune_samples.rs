//! Sample retention: the hourly loop `serve` supervises, and
//! `cargo run --bin kontainer-controller -- task metrics:prune-samples` for a manual pass.

use std::time::Duration;

use async_trait::async_trait;
use loco_rs::{
    Error, Result,
    app::AppContext,
    task::{Task, TaskInfo, Vars},
};
use sea_orm::DatabaseConnection;
use tokio_util::sync::CancellationToken;

use crate::metrics::actions::prune_samples;

pub const PRUNE_EVERY: Duration = Duration::from_secs(60 * 60);

/// Prune once at start, then hourly, until `cancellation`. Failures log and wait for the next hour.
pub async fn run_hourly(database: DatabaseConnection, cancellation: CancellationToken) {
    loop {
        tokio::select! {
            biased;
            () = cancellation.cancelled() => return,
            result = prune_samples::execute(&database) => match result {
                Ok(removed) if removed > 0 => tracing::info!(removed, "pruned node samples"),
                Ok(_) => {}
                Err(error) => tracing::warn!(%error, "node sample pruning failed"),
            },
        }
        tokio::select! {
            () = cancellation.cancelled() => return,
            () = tokio::time::sleep(PRUNE_EVERY) => {}
        }
    }
}

pub struct PruneSamples;

#[async_trait]
impl Task for PruneSamples {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "metrics:prune-samples".to_owned(),
            detail: "Delete node samples older than the metrics retention setting".to_owned(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &Vars) -> Result<()> {
        let removed = prune_samples::execute(&ctx.db)
            .await
            .map_err(|_| Error::string("the database is unavailable; nothing was pruned"))?;
        println!("Pruned {removed} node sample(s).");
        Ok(())
    }
}
