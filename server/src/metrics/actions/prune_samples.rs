use chrono::Duration;
use sea_orm::{
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QuerySelect, QueryTrait,
    sea_query::{Expr, ExprTrait},
};

use crate::{
    nodes::{actions::database_now, entities::node_sample},
    settings::actions::get_settings,
};

pub const PRUNE_BATCH_SIZE: u64 = 10_000;

/// Delete samples older than the retention setting, in batches so one pass never holds a long
/// lock. Returns the number of rows removed.
pub async fn execute(database: &DatabaseConnection) -> Result<u64, DbErr> {
    let settings = get_settings::execute(database).await?;
    let cutoff =
        database_now(database).await? - Duration::days(i64::from(settings.metrics_retention_days));
    let mut removed = 0;
    loop {
        let batch = node_sample::Entity::delete_many()
            .filter(
                Expr::tuple([
                    Expr::col(node_sample::Column::NodeId),
                    Expr::col(node_sample::Column::SampledAt),
                ])
                .in_subquery(
                    node_sample::Entity::find()
                        .select_only()
                        .column(node_sample::Column::NodeId)
                        .column(node_sample::Column::SampledAt)
                        .filter(node_sample::Column::SampledAt.lt(cutoff))
                        .limit(PRUNE_BATCH_SIZE)
                        .into_query(),
                ),
            )
            .exec(database)
            .await?
            .rows_affected;
        removed += batch;
        if batch < PRUNE_BATCH_SIZE {
            return Ok(removed);
        }
    }
}
