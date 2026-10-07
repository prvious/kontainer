use chrono::Utc;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::Set,
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QuerySelect,
    TransactionTrait,
    sea_query::{Expr, LockType},
};

use crate::operations::{
    domain::OperationState,
    entities::{operation, operation_event},
};

pub const RECOVERED_AFTER_RESTART: &str = "recovered_after_restart";

/// Startup recovery (ADR 0009): whatever was `running` when the controller stopped may or may not
/// have happened, so every such operation becomes `uncertain` with a `recovered_after_restart`
/// event, in one transaction and one state update. Returns how many were recovered.
pub async fn execute(database: &DatabaseConnection) -> Result<u64, DbErr> {
    database
        .transaction::<_, u64, DbErr>(|transaction| {
            Box::pin(async move {
                let running = operation::Entity::find()
                    .filter(operation::Column::State.eq(OperationState::Running.as_str()))
                    .lock(LockType::Update)
                    .all(transaction)
                    .await?;
                if running.is_empty() {
                    return Ok(0);
                }
                let now = Utc::now();
                let ids: Vec<_> = running.iter().map(|model| model.id).collect();
                operation::Entity::update_many()
                    .col_expr(
                        operation::Column::State,
                        Expr::value(OperationState::Uncertain.as_str()),
                    )
                    .col_expr(operation::Column::UpdatedAt, Expr::value(now))
                    .filter(operation::Column::Id.is_in(ids.clone()))
                    .exec(transaction)
                    .await?;
                for id in &ids {
                    let sequence = super::next_event_sequence(transaction, *id).await?;
                    operation_event::ActiveModel {
                        operation_id: Set(*id),
                        sequence: Set(sequence),
                        state: Set(OperationState::Uncertain.as_str().to_owned()),
                        error_code: Set(Some(RECOVERED_AFTER_RESTART.to_owned())),
                        created_at: Set(now),
                        ..Default::default()
                    }
                    .insert(transaction)
                    .await?;
                }
                Ok(ids.len() as u64)
            })
        })
        .await
        .map_err(|error| match error {
            sea_orm::TransactionError::Connection(error)
            | sea_orm::TransactionError::Transaction(error) => error,
        })
}
