pub mod admit;
pub mod cancel;
pub mod claim_next;
pub mod finalize;
pub mod get;
pub mod list_events;
pub mod mark_uncertain;
pub mod prune;
pub mod recover_running;
pub mod report_progress;

use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use super::entities::{idempotency_record, operation_event};

// Call only while holding the operation row lock, so concurrent transitions
// cannot allocate the same sequence. Missing history must never be recreated.
async fn next_event_sequence(
    database: &impl ConnectionTrait,
    operation_id: Uuid,
) -> Result<i32, DbErr> {
    operation_event::Entity::find()
        .filter(operation_event::Column::OperationId.eq(operation_id))
        .order_by_desc(operation_event::Column::Sequence)
        .one(database)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("operation history is missing".to_owned()))?
        .sequence
        .checked_add(1)
        .ok_or_else(|| DbErr::Custom("operation event sequence is exhausted".to_owned()))
}

/// Whether `principal_ref` admitted `operation_id`. Every principal-facing read answers
/// "not found" otherwise, so callers cannot probe for other principals' operations.
async fn is_visible(
    database: &impl ConnectionTrait,
    principal_ref: &str,
    operation_id: Uuid,
) -> Result<bool, DbErr> {
    Ok(idempotency_record::Entity::find()
        .filter(idempotency_record::Column::OperationId.eq(operation_id))
        .filter(idempotency_record::Column::PrincipalRef.eq(principal_ref))
        .one(database)
        .await?
        .is_some())
}
