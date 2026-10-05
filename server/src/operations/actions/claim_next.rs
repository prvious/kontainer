use chrono::Utc;
use sea_orm::sea_query::{LockBehavior, LockType};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, TransactionError, TransactionTrait,
};

use crate::operations::{
    domain::{CorruptOperationState, DurableOperation, OperationState},
    entities::{operation, operation_event},
};

#[derive(Debug, thiserror::Error)]
pub enum ClaimOperationError {
    #[error("worker reference is required")]
    EmptyWorkerReference,
    #[error("worker reference exceeds 128 bytes")]
    WorkerReferenceTooLong,
    #[error(transparent)]
    CorruptState(#[from] CorruptOperationState),
    #[error("operation claim persistence failed")]
    Persistence(#[source] DbErr),
}

pub async fn execute(
    database: &DatabaseConnection,
    worker_reference: &str,
) -> Result<Option<DurableOperation>, ClaimOperationError> {
    let worker_reference = worker_reference.trim();
    if worker_reference.is_empty() {
        return Err(ClaimOperationError::EmptyWorkerReference);
    }
    if worker_reference.len() > 128 {
        return Err(ClaimOperationError::WorkerReferenceTooLong);
    }
    let worker_reference = worker_reference.to_owned();

    let claimed = database
        .transaction(|transaction| {
            Box::pin(async move {
                let Some(model) = claimable_query().one(transaction).await? else {
                    return Ok::<_, DbErr>(None);
                };

                let operation_id = model.id;
                let next_sequence = super::next_event_sequence(transaction, operation_id).await?;
                let now = Utc::now();
                let mut active_model: operation::ActiveModel = model.into();
                active_model.state = Set(OperationState::Running.as_str().to_owned());
                active_model.claimed_by = Set(Some(worker_reference));
                active_model.claimed_at = Set(Some(now));
                active_model.updated_at = Set(now);
                let updated = active_model.update(transaction).await?;

                operation_event::ActiveModel {
                    operation_id: Set(operation_id),
                    sequence: Set(next_sequence),
                    state: Set(OperationState::Running.as_str().to_owned()),
                    error_code: Set(None),
                    created_at: Set(now),
                    ..Default::default()
                }
                .insert(transaction)
                .await?;

                Ok(Some(updated))
            })
        })
        .await
        .map_err(transaction_error)?;

    claimed
        .map(TryInto::try_into)
        .transpose()
        .map_err(ClaimOperationError::CorruptState)
}

pub(crate) fn claimable_query() -> sea_orm::Select<operation::Entity> {
    operation::Entity::find()
        .filter(operation::Column::State.eq(OperationState::Recoverable.as_str()))
        .order_by_asc(operation::Column::CreatedAt)
        .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
}

fn transaction_error(error: TransactionError<DbErr>) -> ClaimOperationError {
    match error {
        TransactionError::Connection(error) | TransactionError::Transaction(error) => {
            ClaimOperationError::Persistence(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{DbBackend, QueryTrait};

    use super::claimable_query;

    #[test]
    fn claim_query_uses_postgresql_skip_locked() {
        let statement = claimable_query().build(DbBackend::Postgres).to_string();

        assert!(statement.ends_with("FOR UPDATE SKIP LOCKED"));
        assert!(statement.contains("\"operations\".\"state\" = 'recoverable'"));
    }
}
