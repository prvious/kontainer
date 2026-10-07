use chrono::Utc;
use sea_orm::sea_query::LockType;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, QueryFilter, QuerySelect, TransactionError, TransactionTrait,
};
use uuid::Uuid;

use crate::auth::{
    domain::{RevocationReason, actions},
    entities::{refresh_credential, security_audit_intent, session_family},
};

#[derive(Debug, thiserror::Error)]
pub enum RevokeSessionError {
    #[error("session not found")]
    NotFound,
    #[error("session revocation persistence failed")]
    Persistence(#[source] DbErr),
}

pub async fn execute(
    database: &DatabaseConnection,
    family_id: Uuid,
    reason: RevocationReason,
) -> Result<(), RevokeSessionError> {
    database
        .transaction(|transaction| {
            Box::pin(async move { revoke_in_transaction(transaction, family_id, reason).await })
        })
        .await
        .map_err(|error| match error {
            TransactionError::Connection(error) => RevokeSessionError::Persistence(error),
            TransactionError::Transaction(error) => error,
        })
}

pub(crate) async fn revoke_in_transaction<C>(
    database: &C,
    family_id: Uuid,
    reason: RevocationReason,
) -> Result<(), RevokeSessionError>
where
    C: ConnectionTrait,
{
    let family = session_family::Entity::find_by_id(family_id)
        .lock(LockType::Update)
        .one(database)
        .await
        .map_err(RevokeSessionError::Persistence)?
        .ok_or(RevokeSessionError::NotFound)?;

    // Logout is idempotent: an already-revoked family stays revoked without
    // recording duplicate audit rows on retry.
    if family.revoked_at.is_some() {
        return Ok(());
    }

    let now = Utc::now();
    let mut family_model: session_family::ActiveModel = family.clone().into();
    family_model.revoked_at = Set(Some(now));
    family_model.revoked_reason = Set(Some(reason.as_str().to_owned()));
    family_model.updated_at = Set(now);
    family_model
        .update(database)
        .await
        .map_err(RevokeSessionError::Persistence)?;

    refresh_credential::Entity::update_many()
        .col_expr(
            refresh_credential::Column::RevokedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .filter(refresh_credential::Column::FamilyId.eq(family_id))
        .filter(refresh_credential::Column::RevokedAt.is_null())
        .exec(database)
        .await
        .map_err(RevokeSessionError::Persistence)?;

    security_audit_intent::ActiveModel {
        id: Set(Uuid::now_v7()),
        principal_id: Set(family.principal_id),
        family_id: Set(family_id),
        action: Set(actions::REVOKE.to_owned()),
        state: Set(actions::PENDING.to_owned()),
        created_at: Set(now),
    }
    .insert(database)
    .await
    .map_err(RevokeSessionError::Persistence)?;

    Ok(())
}
