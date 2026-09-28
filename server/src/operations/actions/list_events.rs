use sea_orm::{
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use uuid::Uuid;

use super::is_visible;
use crate::{
    auth::domain::PrincipalContext,
    operations::{
        domain::{CorruptOperationState, OperationEventView, OperationState},
        entities::operation_event,
    },
};

pub const MAX_EVENT_PAGE: u64 = 100;

#[derive(Debug, thiserror::Error)]
pub enum ListEventsError {
    #[error("operation not found")]
    NotFound,
    #[error("limit must be between 1 and 100")]
    InvalidLimit,
    #[error(transparent)]
    CorruptState(#[from] CorruptOperationState),
    #[error("operation event persistence failed")]
    Persistence(#[from] DbErr),
}

/// Events with `sequence > after`, oldest first, at most `limit`.
pub async fn execute(
    database: &DatabaseConnection,
    principal: &PrincipalContext,
    operation_id: Uuid,
    after: i32,
    limit: u64,
) -> Result<Vec<OperationEventView>, ListEventsError> {
    if !(1..=MAX_EVENT_PAGE).contains(&limit) {
        return Err(ListEventsError::InvalidLimit);
    }
    if !is_visible(database, &principal.reference(), operation_id).await? {
        return Err(ListEventsError::NotFound);
    }
    operation_event::Entity::find()
        .filter(operation_event::Column::OperationId.eq(operation_id))
        .filter(operation_event::Column::Sequence.gt(after))
        .order_by_asc(operation_event::Column::Sequence)
        .limit(limit)
        .all(database)
        .await?
        .into_iter()
        .map(|event| {
            Ok(OperationEventView {
                sequence: event.sequence,
                state: OperationState::try_from(event.state.as_str())?,
                error_code: event.error_code,
                message: event.message,
                detail: event.detail,
                created_at: event.created_at,
            })
        })
        .collect()
}
