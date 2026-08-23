use sea_orm::{
    AccessMode, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, IsolationLevel, QueryFilter,
    QueryOrder, TransactionError, TransactionTrait,
};
use uuid::Uuid;

use crate::operations::{
    domain::{CorruptOperationState, OperationSnapshot},
    entities::{operation, operation_event},
};

#[derive(Debug, thiserror::Error)]
pub enum GetOperationError {
    #[error("operation not found")]
    NotFound,
    #[error("operation history is missing its initial event")]
    MissingEvent,
    #[error(transparent)]
    CorruptState(#[from] CorruptOperationState),
    #[error("operation polling persistence failed")]
    Persistence(#[source] DbErr),
}

pub async fn execute(
    database: &DatabaseConnection,
    operation_id: Uuid,
) -> Result<OperationSnapshot, GetOperationError> {
    database
        .transaction_with_config(
            |transaction| {
                Box::pin(async move {
                    let operation = operation::Entity::find_by_id(operation_id)
                        .one(transaction)
                        .await
                        .map_err(GetOperationError::Persistence)?
                        .ok_or(GetOperationError::NotFound)?
                        .try_into()?;
                    let last_event = operation_event::Entity::find()
                        .filter(operation_event::Column::OperationId.eq(operation_id))
                        .order_by_desc(operation_event::Column::Sequence)
                        .one(transaction)
                        .await
                        .map_err(GetOperationError::Persistence)?
                        .ok_or(GetOperationError::MissingEvent)?;

                    Ok(OperationSnapshot {
                        operation,
                        last_event_sequence: last_event.sequence,
                        last_error_code: last_event.error_code,
                    })
                })
            },
            Some(IsolationLevel::RepeatableRead),
            Some(AccessMode::ReadOnly),
        )
        .await
        .map_err(|error| match error {
            TransactionError::Connection(error) => GetOperationError::Persistence(error),
            TransactionError::Transaction(error) => error,
        })
}
