use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, DatabaseConnection, DbErr, EntityTrait, QuerySelect,
    TransactionError, TransactionTrait, sea_query::LockType,
};
use uuid::Uuid;

use crate::operations::{
    domain::OperationState,
    entities::{operation, operation_event},
};

#[derive(Debug, thiserror::Error)]
pub enum ReportProgressError {
    #[error("operation not found")]
    NotFound,
    #[error("only a running operation reports progress")]
    NotRunning,
    #[error("operation progress persistence failed")]
    Persistence(#[from] DbErr),
}

/// Append a `running` event carrying the kind's progress document.
pub async fn execute(
    database: &DatabaseConnection,
    operation_id: Uuid,
    detail: serde_json::Value,
) -> Result<(), ReportProgressError> {
    database
        .transaction(|transaction| {
            Box::pin(async move {
                let model = operation::Entity::find_by_id(operation_id)
                    .lock(LockType::Update)
                    .one(transaction)
                    .await?
                    .ok_or(ReportProgressError::NotFound)?;
                if model.state != OperationState::Running.as_str() {
                    return Err(ReportProgressError::NotRunning);
                }
                let sequence = super::next_event_sequence(transaction, operation_id).await?;
                let now = Utc::now();
                operation_event::ActiveModel {
                    operation_id: Set(operation_id),
                    sequence: Set(sequence),
                    state: Set(OperationState::Running.as_str().to_owned()),
                    detail: Set(Some(detail)),
                    created_at: Set(now),
                    ..Default::default()
                }
                .insert(transaction)
                .await?;
                let mut active: operation::ActiveModel = model.into();
                active.updated_at = Set(now);
                active.update(transaction).await?;
                Ok(())
            })
        })
        .await
        .map_err(|error| match error {
            TransactionError::Connection(error) => ReportProgressError::Persistence(error),
            TransactionError::Transaction(error) => error,
        })
}
