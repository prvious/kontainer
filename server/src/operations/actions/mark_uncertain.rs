use chrono::Utc;
use sea_orm::sea_query::LockType;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, TransactionError, TransactionTrait,
};
use uuid::Uuid;

use crate::operations::{
    domain::{CorruptOperationState, DurableOperation, OperationState},
    entities::{operation, operation_event},
};

#[derive(Debug, thiserror::Error)]
pub enum MarkUncertainError {
    #[error("operation not found")]
    NotFound,
    #[error("only a running operation can become uncertain")]
    InvalidState,
    #[error("error code is required")]
    EmptyErrorCode,
    #[error("error code exceeds 64 bytes")]
    ErrorCodeTooLong,
    #[error(transparent)]
    CorruptState(#[from] CorruptOperationState),
    #[error("operation transition persistence failed")]
    Persistence(#[source] DbErr),
}

#[derive(Debug, thiserror::Error)]
enum TransitionError {
    #[error("operation not found")]
    NotFound,
    #[error("invalid operation state")]
    InvalidState,
    #[error(transparent)]
    Database(#[from] DbErr),
}

pub async fn execute(
    database: &DatabaseConnection,
    operation_id: Uuid,
    error_code: &str,
) -> Result<DurableOperation, MarkUncertainError> {
    let error_code = error_code.trim();
    if error_code.is_empty() {
        return Err(MarkUncertainError::EmptyErrorCode);
    }
    if error_code.len() > 64 {
        return Err(MarkUncertainError::ErrorCodeTooLong);
    }
    let error_code = error_code.to_owned();

    let updated = database
        .transaction(|transaction| {
            Box::pin(async move {
                let model = operation::Entity::find_by_id(operation_id)
                    .lock(LockType::Update)
                    .one(transaction)
                    .await?
                    .ok_or(TransitionError::NotFound)?;
                if model.state != OperationState::Running.as_str() {
                    return Err(TransitionError::InvalidState);
                }

                let next_sequence = operation_event::Entity::find()
                    .filter(operation_event::Column::OperationId.eq(operation_id))
                    .order_by_desc(operation_event::Column::Sequence)
                    .one(transaction)
                    .await?
                    .map_or(1, |event| event.sequence + 1);
                let now = Utc::now();
                let mut active_model: operation::ActiveModel = model.into();
                active_model.state = Set(OperationState::Uncertain.as_str().to_owned());
                active_model.updated_at = Set(now);
                let updated = active_model.update(transaction).await?;

                operation_event::ActiveModel {
                    operation_id: Set(operation_id),
                    sequence: Set(next_sequence),
                    state: Set(OperationState::Uncertain.as_str().to_owned()),
                    error_code: Set(Some(error_code)),
                    created_at: Set(now),
                    ..Default::default()
                }
                .insert(transaction)
                .await?;

                Ok(updated)
            })
        })
        .await
        .map_err(map_transaction_error)?;

    updated.try_into().map_err(MarkUncertainError::CorruptState)
}

fn map_transaction_error(error: TransactionError<TransitionError>) -> MarkUncertainError {
    match error {
        TransactionError::Connection(error) => MarkUncertainError::Persistence(error),
        TransactionError::Transaction(TransitionError::NotFound) => MarkUncertainError::NotFound,
        TransactionError::Transaction(TransitionError::InvalidState) => {
            MarkUncertainError::InvalidState
        }
        TransactionError::Transaction(TransitionError::Database(error)) => {
            MarkUncertainError::Persistence(error)
        }
    }
}
