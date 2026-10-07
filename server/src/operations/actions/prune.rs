use chrono::{Duration, Utc};
use sea_orm::{
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QuerySelect, QueryTrait,
    TransactionTrait,
};
use uuid::Uuid;

use crate::operations::{
    domain::OperationState,
    entities::{audit_intent, idempotency_record, operation},
};

const TERMINAL: [&str; 3] = [
    OperationState::Succeeded.as_str(),
    OperationState::Failed.as_str(),
    OperationState::Cancelled.as_str(),
];

/// Terminal operations are kept this long.
pub const RETENTION: Duration = Duration::days(7);
pub const PRUNE_BATCH: u64 = 1_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PruneReport {
    pub operations: u64,
    pub idempotency_records: u64,
}

/// Delete terminal operations older than [`RETENTION`] (their idempotency records, audit intents,
/// and events with them) and expired idempotency records of terminal operations, in batches.
pub async fn execute(database: &DatabaseConnection) -> Result<PruneReport, DbErr> {
    let cutoff = Utc::now() - RETENTION;
    let mut report = PruneReport::default();
    loop {
        let batch: Vec<Uuid> = operation::Entity::find()
            .select_only()
            .column(operation::Column::Id)
            .filter(operation::Column::State.is_in(TERMINAL))
            .filter(operation::Column::UpdatedAt.lt(cutoff))
            .limit(PRUNE_BATCH)
            .into_tuple()
            .all(database)
            .await?;
        if batch.is_empty() {
            break;
        }
        let size = batch.len() as u64;
        database
            .transaction::<_, (), DbErr>(|transaction| {
                Box::pin(async move {
                    audit_intent::Entity::delete_many()
                        .filter(audit_intent::Column::OperationId.is_in(batch.clone()))
                        .exec(transaction)
                        .await?;
                    idempotency_record::Entity::delete_many()
                        .filter(idempotency_record::Column::OperationId.is_in(batch.clone()))
                        .exec(transaction)
                        .await?;
                    operation::Entity::delete_many()
                        .filter(operation::Column::Id.is_in(batch))
                        .exec(transaction)
                        .await?;
                    Ok(())
                })
            })
            .await
            .map_err(|error| match error {
                sea_orm::TransactionError::Connection(error)
                | sea_orm::TransactionError::Transaction(error) => error,
            })?;
        report.operations += size;
        if size < PRUNE_BATCH {
            break;
        }
    }
    report.idempotency_records = idempotency_record::Entity::delete_many()
        .filter(idempotency_record::Column::ExpiresAt.lt(Utc::now()))
        .filter(
            idempotency_record::Column::OperationId.in_subquery(
                operation::Entity::find()
                    .select_only()
                    .column(operation::Column::Id)
                    .filter(operation::Column::State.is_in(TERMINAL))
                    .into_query(),
            ),
        )
        .exec(database)
        .await?
        .rows_affected;
    Ok(report)
}
