use chrono::Utc;
use sea_orm::sea_query::LockType;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, DatabaseConnection, DbErr, EntityTrait, QuerySelect,
    TransactionError, TransactionTrait,
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
    Persistence(#[from] DbErr),
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
                    .ok_or(MarkUncertainError::NotFound)?;
                if model.state != OperationState::Running.as_str() {
                    return Err(MarkUncertainError::InvalidState);
                }

                let next_sequence = super::next_event_sequence(transaction, operation_id).await?;
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
        .map_err(|error| match error {
            TransactionError::Connection(error) => MarkUncertainError::Persistence(error),
            TransactionError::Transaction(error) => error,
        })?;

    updated.try_into().map_err(MarkUncertainError::CorruptState)
}
