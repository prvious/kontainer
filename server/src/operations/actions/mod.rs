pub mod admit;
pub mod claim_next;
pub mod get;
pub mod mark_uncertain;

use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use super::entities::operation_event;

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
