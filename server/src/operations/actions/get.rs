use sea_orm::{
    AccessMode, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, IsolationLevel, QueryFilter,
    QueryOrder, TransactionError, TransactionTrait,
};
use uuid::Uuid;

use super::is_visible;
use crate::{
    auth::domain::PrincipalContext,
    operations::{
        domain::{CorruptOperationState, OperationSnapshot, OperationState},
        entities::{operation, operation_event},
    },
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
    principal: &PrincipalContext,
    operation_id: Uuid,
) -> Result<OperationSnapshot, GetOperationError> {
    let principal_ref = principal.reference();
    database
        .transaction_with_config(
            |transaction| {
                Box::pin(async move {
                    // Visibility derives from the immutable admission-principal
                    // binding. A missing binding returns the same NotFound as
                    // a nonexistent operation so callers cannot probe for
                    // another principal's operations.
                    if !is_visible(transaction, &principal_ref, operation_id)
                        .await
                        .map_err(GetOperationError::Persistence)?
                    {
                        return Err(GetOperationError::NotFound);
                    }
                    let operation: crate::operations::domain::DurableOperation =
                        operation::Entity::find_by_id(operation_id)
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
                    let progress = operation_event::Entity::find()
                        .filter(operation_event::Column::OperationId.eq(operation_id))
                        .filter(operation_event::Column::Detail.is_not_null())
                        .order_by_desc(operation_event::Column::Sequence)
                        .one(transaction)
                        .await
                        .map_err(GetOperationError::Persistence)?
                        .and_then(|event| event.detail);
                    let error_message = if operation.state == OperationState::Failed {
                        last_event.message.clone()
                    } else {
                        None
                    };

                    Ok(OperationSnapshot {
                        operation,
                        last_event_sequence: last_event.sequence,
                        last_error_code: last_event.error_code,
                        progress,
                        error_message,
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
