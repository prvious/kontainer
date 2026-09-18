use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    TransactionError, TransactionTrait,
};
use uuid::Uuid;

use crate::auth::{
    credentials::{AccessCredential, RefreshCredential},
    domain::{IssuedSession, PrincipalStatus, absolute_expiry, access_expiry, actions},
    entities::{principal, refresh_credential, security_audit_intent, session_family},
};

#[derive(Debug, thiserror::Error)]
pub enum CreateSessionError {
    #[error("principal not found")]
    PrincipalNotFound,
    #[error("principal is not active")]
    PrincipalInactive,
    #[error("session creation persistence failed")]
    Persistence(#[source] DbErr),
}

pub async fn execute(
    database: &DatabaseConnection,
    principal_id: Uuid,
) -> Result<IssuedSession, CreateSessionError> {
    database
        .transaction(|transaction| {
            Box::pin(async move { create_in_transaction(transaction, principal_id).await })
        })
        .await
        .map_err(|error| match error {
            TransactionError::Connection(error) => CreateSessionError::Persistence(error),
            TransactionError::Transaction(error) => error,
        })
}

/// Transaction-scoped creation so later login flows can compose principal
/// activation and session issuance atomically.
pub async fn create_in_transaction<C>(
    database: &C,
    principal_id: Uuid,
) -> Result<IssuedSession, CreateSessionError>
where
    C: ConnectionTrait,
{
    let principal = principal::Entity::find_by_id(principal_id)
        .one(database)
        .await
        .map_err(CreateSessionError::Persistence)?
        .ok_or(CreateSessionError::PrincipalNotFound)?;
    if principal.status != PrincipalStatus::Active.as_str() {
        return Err(CreateSessionError::PrincipalInactive);
    }

    let now = Utc::now();
    let access = AccessCredential::generate();
    let refresh = RefreshCredential::generate();
    let family_id = Uuid::now_v7();
    let access_expires_at = access_expiry(now);
    let absolute_expires_at = absolute_expiry(now);

    session_family::ActiveModel {
        id: Set(family_id),
        principal_id: Set(principal_id),
        access_hash: Set(access.credential_hash()),
        access_expires_at: Set(access_expires_at),
        absolute_expires_at: Set(absolute_expires_at),
        revoked_at: Set(None),
        revoked_reason: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(database)
    .await
    .map_err(CreateSessionError::Persistence)?;

    refresh_credential::ActiveModel {
        id: Set(refresh.public_id()),
        family_id: Set(family_id),
        generation: Set(0),
        credential_hash: Set(refresh.credential_hash()),
        expires_at: Set(absolute_expires_at),
        consumed_at: Set(None),
        replaced_by: Set(None),
        revoked_at: Set(None),
        created_at: Set(now),
    }
    .insert(database)
    .await
    .map_err(CreateSessionError::Persistence)?;

    security_audit_intent::ActiveModel {
        id: Set(Uuid::now_v7()),
        principal_id: Set(principal_id),
        family_id: Set(family_id),
        action: Set(actions::CREATE.to_owned()),
        state: Set(actions::PENDING.to_owned()),
        created_at: Set(now),
    }
    .insert(database)
    .await
    .map_err(CreateSessionError::Persistence)?;

    Ok(IssuedSession {
        principal_id,
        family_id,
        access_plaintext: access.plaintext(),
        refresh_plaintext: refresh.plaintext(),
        access_expires_at,
        refresh_expires_at: absolute_expires_at,
    })
}
