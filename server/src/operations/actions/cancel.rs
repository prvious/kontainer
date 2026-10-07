use sea_orm::{
    DatabaseConnection, DbErr, EntityTrait, QuerySelect, TransactionError, TransactionTrait,
    sea_query::LockType,
};
use uuid::Uuid;

use super::{
    finalize::{FinalizeError, Outcome, finalize_in},
    is_visible,
};
use crate::{
    auth::domain::PrincipalContext,
    operations::{
        domain::{CorruptOperationState, DurableOperation, OperationState},
        entities::operation,
    },
};

#[derive(Debug, thiserror::Error)]
pub enum CancelOperationError {
    #[error("operation not found")]
    NotFound,
    #[error("only an operation that has not started can be cancelled")]
    NotCancellable,
    #[error(transparent)]
    CorruptState(#[from] CorruptOperationState),
    #[error("operation cancellation persistence failed")]
    Persistence(#[from] DbErr),
}

/// Cancel an operation the principal admitted, only while it is still `recoverable` (ADR 0009:
/// running operations finish). The claim query skips locked rows, so a cancel racing a claim
/// resolves to exactly one of them.
pub async fn execute(
    database: &DatabaseConnection,
    principal: &PrincipalContext,
    operation_id: Uuid,
) -> Result<DurableOperation, CancelOperationError> {
    let principal_ref = principal.reference();
    database
        .transaction(|transaction| {
            Box::pin(async move {
                if !is_visible(transaction, &principal_ref, operation_id).await? {
                    return Err(CancelOperationError::NotFound);
                }
                // Lock before checking, so a claim cannot start the operation in between.
                let model = operation::Entity::find_by_id(operation_id)
                    .lock(LockType::Update)
                    .one(transaction)
                    .await?
                    .ok_or(CancelOperationError::NotFound)?;
                if OperationState::try_from(model.state.as_str())? != OperationState::Recoverable {
                    return Err(CancelOperationError::NotCancellable);
                }
                match finalize_in(transaction, operation_id, Outcome::Cancelled).await {
                    Ok(operation) if operation.state == OperationState::Cancelled => Ok(operation),
                    Ok(_) | Err(FinalizeError::AlreadyTerminal) => {
                        Err(CancelOperationError::NotCancellable)
                    }
                    Err(FinalizeError::NotFound) => Err(CancelOperationError::NotFound),
                    Err(FinalizeError::CorruptState(error)) => Err(error.into()),
                    Err(FinalizeError::Persistence(error)) => Err(error.into()),
                }
            })
        })
        .await
        .map_err(|error| match error {
            TransactionError::Connection(error) => CancelOperationError::Persistence(error),
            TransactionError::Transaction(error) => error,
        })
}
