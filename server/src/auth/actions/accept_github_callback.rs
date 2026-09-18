use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QuerySelect, TransactionError, TransactionTrait, sea_query::LockType,
};

use crate::auth::{entities::github_login_attempt, github::state_hash};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackOutcome {
    Ready { attempt_id: uuid::Uuid },
    Denied,
}

#[derive(Debug, thiserror::Error)]
pub enum CallbackError {
    #[error("callback persistence failed")]
    Persistence(#[source] sea_orm::DbErr),
}

fn valid_state(value: &str) -> bool {
    // Plaintext states are 43 URL-safe characters; bound the input before
    // hashing so oversized values share the denial path.
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn valid_code(value: &str) -> bool {
    // GitHub authorization codes are short opaque strings. The bound keeps
    // the persisted value small; content is never interpreted here.
    !value.is_empty() && value.len() <= 512 && value.bytes().all(|byte| byte.is_ascii_graphic())
}

async fn consume<C>(
    database: &C,
    attempt: &github_login_attempt::Model,
    now: chrono::DateTime<Utc>,
) -> Result<(), CallbackError>
where
    C: sea_orm::ConnectionTrait,
{
    let mut consumed: github_login_attempt::ActiveModel = attempt.clone().into();
    consumed.status = Set("consumed".to_owned());
    consumed.consumed_at = Set(Some(now));
    consumed.updated_at = Set(now);
    consumed
        .update(database)
        .await
        .map_err(CallbackError::Persistence)?;
    Ok(())
}

/// Consume the OAuth state exactly once and hold the provider code for the
/// bound desktop redemption. Identity still comes only from code exchange
/// at redemption time; query fields never assert it.
pub async fn execute(
    database: &DatabaseConnection,
    state: &str,
    code: Option<&str>,
    provider_error: Option<&str>,
) -> Result<CallbackOutcome, CallbackError> {
    if !valid_state(state) {
        return Ok(CallbackOutcome::Denied);
    }
    let hash = state_hash(state);
    let denied_by_provider = provider_error.is_some_and(|error| !error.is_empty());
    let code = code.map(str::to_owned);

    database
        .transaction(|transaction| {
            Box::pin(async move {
                let attempt = github_login_attempt::Entity::find()
                    .filter(github_login_attempt::Column::StateHash.eq(hash))
                    .lock(LockType::Update)
                    .one(transaction)
                    .await
                    .map_err(CallbackError::Persistence)?;
                let Some(attempt) = attempt else {
                    return Ok(CallbackOutcome::Denied);
                };
                let now = Utc::now();
                if attempt.status != "pending" {
                    // A ready attempt already holds a code; a second
                    // callback is a replay. Consumed attempts stay denied.
                    return Ok(CallbackOutcome::Denied);
                }
                if attempt.expires_at <= now
                    || denied_by_provider
                    || code.as_deref().is_none_or(|code| !valid_code(code))
                {
                    consume(transaction, &attempt, now).await?;
                    return Ok(CallbackOutcome::Denied);
                }
                let mut ready: github_login_attempt::ActiveModel = attempt.clone().into();
                ready.status = Set("ready".to_owned());
                ready.provider_code = Set(Some(code.unwrap_or_default()));
                ready.updated_at = Set(now);
                ready
                    .update(transaction)
                    .await
                    .map_err(CallbackError::Persistence)?;
                Ok(CallbackOutcome::Ready {
                    attempt_id: attempt.id,
                })
            })
        })
        .await
        .map_err(|error| match error {
            TransactionError::Connection(error) => CallbackError::Persistence(error),
            TransactionError::Transaction(error) => error,
        })
}
