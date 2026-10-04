use chrono::Utc;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QuerySelect,
    TransactionError, TransactionTrait,
    sea_query::{Expr, LockType},
};
use uuid::Uuid;

use crate::operations::{
    domain::{CorruptOperationState, DurableOperation, OperationState},
    entities::{audit_intent, operation, operation_event},
};

const MAX_CODE_LEN: usize = 64;
const MAX_MESSAGE_LEN: usize = 1024;

/// How an operation ended.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Succeeded {
        detail: Option<serde_json::Value>,
    },
    Failed {
        code: String,
        message: String,
        detail: Option<serde_json::Value>,
    },
    Cancelled,
}

impl Outcome {
    const fn state(&self) -> OperationState {
        match self {
            Self::Succeeded { .. } => OperationState::Succeeded,
            Self::Failed { .. } => OperationState::Failed,
            Self::Cancelled => OperationState::Cancelled,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FinalizeError {
    #[error("operation not found")]
    NotFound,
    #[error("operation is already terminal")]
    AlreadyTerminal,
    #[error(transparent)]
    CorruptState(#[from] CorruptOperationState),
    #[error("operation finalization persistence failed")]
    Persistence(#[from] DbErr),
}

/// Write the one terminal event and state, stamp `finished_at`, and settle the audit intent.
pub async fn execute(
    database: &DatabaseConnection,
    operation_id: Uuid,
    outcome: Outcome,
) -> Result<DurableOperation, FinalizeError> {
    #[cfg(test)]
    if injected::take(operation_id) {
        return Err(FinalizeError::Persistence(DbErr::Custom(
            "injected finalize failure".to_owned(),
        )));
    }
    database
        .transaction(|transaction| {
            Box::pin(async move { finalize_in(transaction, operation_id, outcome).await })
        })
        .await
        .map_err(|error| match error {
            TransactionError::Connection(error) => FinalizeError::Persistence(error),
            TransactionError::Transaction(error) => error,
        })
}

/// [`execute`] inside a caller's transaction that already holds no conflicting row lock.
pub(crate) async fn finalize_in(
    transaction: &impl ConnectionTrait,
    operation_id: Uuid,
    outcome: Outcome,
) -> Result<DurableOperation, FinalizeError> {
    let model = operation::Entity::find_by_id(operation_id)
        .lock(LockType::Update)
        .one(transaction)
        .await?
        .ok_or(FinalizeError::NotFound)?;
    if OperationState::try_from(model.state.as_str())?.is_terminal() {
        return Err(FinalizeError::AlreadyTerminal);
    }
    let sequence = super::next_event_sequence(transaction, operation_id).await?;
    let now = Utc::now();
    let state = outcome.state();
    let (error_code, message, detail) = match outcome {
        Outcome::Succeeded { detail } => (None, None, detail),
        Outcome::Failed {
            code,
            message,
            detail,
        } => (
            Some(bounded(&code, MAX_CODE_LEN)),
            Some(bounded(&message, MAX_MESSAGE_LEN)),
            detail,
        ),
        Outcome::Cancelled => (None, None, None),
    };
    let mut active: operation::ActiveModel = model.into();
    active.state = Set(state.as_str().to_owned());
    active.updated_at = Set(now);
    active.finished_at = Set(Some(now));
    let updated = active.update(transaction).await?;
    operation_event::ActiveModel {
        operation_id: Set(operation_id),
        sequence: Set(sequence),
        state: Set(state.as_str().to_owned()),
        error_code: Set(error_code),
        message: Set(message),
        detail: Set(detail),
        created_at: Set(now),
        ..Default::default()
    }
    .insert(transaction)
    .await?;
    audit_intent::Entity::update_many()
        .col_expr(audit_intent::Column::State, Expr::value(state.as_str()))
        .filter(audit_intent::Column::OperationId.eq(operation_id))
        .exec(transaction)
        .await?;
    Ok(updated.try_into()?)
}

fn bounded(value: &str, limit: usize) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= limit {
        return trimmed.to_owned();
    }
    let mut end = limit;
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    trimmed[..end].to_owned()
}

/// A failure point for tests: the next `execute` for an operation fails with a persistence error
/// before touching the database. Keyed by operation id, so tests running at once don't interfere.
#[cfg(test)]
pub(crate) mod injected {
    use std::{collections::BTreeSet, sync::Mutex};

    use uuid::Uuid;

    static FAIL_NEXT: Mutex<BTreeSet<Uuid>> = Mutex::new(BTreeSet::new());

    pub(crate) fn fail_next(operation_id: Uuid) {
        FAIL_NEXT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(operation_id);
    }

    pub(super) fn take(operation_id: Uuid) -> bool {
        FAIL_NEXT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&operation_id)
    }
}

#[cfg(test)]
mod tests {
    use super::bounded;

    #[test]
    fn codes_and_messages_are_trimmed_to_their_bounds_on_char_boundaries() {
        assert_eq!(bounded("  timeout ", 64), "timeout");
        assert_eq!(bounded(&"a".repeat(70), 64).len(), 64);
        assert_eq!(bounded("ééé", 3), "é");
    }
}
