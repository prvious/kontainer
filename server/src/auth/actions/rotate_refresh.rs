use chrono::Utc;
use sea_orm::sea_query::LockType;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, QuerySelect, TransactionError, TransactionTrait,
};

use crate::auth::{
    credentials::{AccessCredential, RefreshCredential, hash_presented},
    domain::{IssuedSession, PrincipalStatus, RevocationReason, access_expiry, actions},
    entities::{principal, refresh_credential, security_audit_intent, session_family},
};

#[derive(Debug, thiserror::Error)]
pub enum RotateRefreshError {
    #[error("refresh credential is not valid")]
    Unauthorized,
    #[error("refresh credential was already used")]
    ReuseDetected,
    #[error("refresh rotation persistence failed")]
    Persistence(#[source] DbErr),
}

/// Outcome committed inside the transaction. Reuse must commit revocation and
/// only then surface as an error, so the transaction closure returns the
/// outcome instead of failing outright.
enum RotationOutcome {
    Rotated(IssuedSession),
    Reused,
    Unauthorized,
}

pub async fn execute(
    database: &DatabaseConnection,
    presented: &str,
) -> Result<IssuedSession, RotateRefreshError> {
    // Hash first and resolve the row by its indexed hash: malformed,
    // wrong-kind, and unknown credentials share one lookup path, and the
    // verifier comparison happens in the database, not in this process.
    let presented_hash = hash_presented(presented);

    let outcome = database
        .transaction(|transaction| {
            Box::pin(async move { rotate_in_transaction(transaction, &presented_hash).await })
        })
        .await
        .map_err(|error| match error {
            TransactionError::Connection(error) => RotateRefreshError::Persistence(error),
            TransactionError::Transaction(error) => error,
        })?;

    match outcome {
        RotationOutcome::Rotated(issued) => Ok(issued),
        RotationOutcome::Reused => Err(RotateRefreshError::ReuseDetected),
        RotationOutcome::Unauthorized => Err(RotateRefreshError::Unauthorized),
    }
}

async fn rotate_in_transaction<C>(
    database: &C,
    presented_hash: &str,
) -> Result<RotationOutcome, RotateRefreshError>
where
    C: sea_orm::ConnectionTrait,
{
    let Some(presented_row) = refresh_credential::Entity::find()
        .filter(refresh_credential::Column::CredentialHash.eq(presented_hash.to_owned()))
        .one(database)
        .await
        .map_err(RotateRefreshError::Persistence)?
    else {
        return Ok(RotationOutcome::Unauthorized);
    };
    let presented_id = presented_row.id;

    // Lock the family first so concurrent refreshes serialize here in a
    // consistent order; the loser then observes consumption and revokes.
    let Some(family) = session_family::Entity::find_by_id(presented_row.family_id)
        .lock(LockType::Update)
        .one(database)
        .await
        .map_err(RotateRefreshError::Persistence)?
    else {
        return Ok(RotationOutcome::Unauthorized);
    };
    if family.revoked_at.is_some() {
        return Ok(RotationOutcome::Unauthorized);
    }

    // Re-read the presented row after acquiring the family lock: a concurrent
    // rotation may have consumed it while this transaction waited.
    let Some(current) = refresh_credential::Entity::find_by_id(presented_id)
        .one(database)
        .await
        .map_err(RotateRefreshError::Persistence)?
    else {
        return Ok(RotationOutcome::Unauthorized);
    };

    let now = Utc::now();
    // Reuse revokes the family before any status or expiry check: a suspended
    // principal must not shelter a stolen credential, and reactivation must
    // never resurrect the rotated successor.
    if current.consumed_at.is_some() {
        revoke_family_for_reuse(database, &family, now).await?;
        return Ok(RotationOutcome::Reused);
    }

    if current.revoked_at.is_some() || current.expires_at <= now {
        return Ok(RotationOutcome::Unauthorized);
    }
    if family.absolute_expires_at <= now {
        return Ok(RotationOutcome::Unauthorized);
    }

    let owner = principal::Entity::find_by_id(family.principal_id)
        .one(database)
        .await
        .map_err(RotateRefreshError::Persistence)?;
    let Some(owner) = owner else {
        return Ok(RotationOutcome::Unauthorized);
    };
    if owner.status != PrincipalStatus::Active.as_str() {
        return Ok(RotationOutcome::Unauthorized);
    }

    let access = AccessCredential::generate();
    let successor = RefreshCredential::generate();
    let successor_id = successor.public_id();
    let access_expires_at = access_expiry(now);

    let mut family_model: session_family::ActiveModel = family.clone().into();
    family_model.access_hash = Set(access.credential_hash());
    family_model.access_expires_at = Set(access_expires_at);
    family_model.updated_at = Set(now);
    family_model
        .update(database)
        .await
        .map_err(RotateRefreshError::Persistence)?;

    refresh_credential::ActiveModel {
        id: Set(successor_id),
        family_id: Set(family.id),
        generation: Set(current.generation + 1),
        credential_hash: Set(successor.credential_hash()),
        expires_at: Set(family.absolute_expires_at),
        consumed_at: Set(None),
        replaced_by: Set(None),
        revoked_at: Set(None),
        created_at: Set(now),
    }
    .insert(database)
    .await
    .map_err(RotateRefreshError::Persistence)?;

    let mut consumed: refresh_credential::ActiveModel = current.into();
    consumed.consumed_at = Set(Some(now));
    consumed.replaced_by = Set(Some(successor_id));
    consumed
        .update(database)
        .await
        .map_err(RotateRefreshError::Persistence)?;

    security_audit_intent::ActiveModel {
        id: Set(uuid::Uuid::now_v7()),
        principal_id: Set(family.principal_id),
        family_id: Set(family.id),
        action: Set(actions::ROTATE.to_owned()),
        state: Set(actions::PENDING.to_owned()),
        created_at: Set(now),
    }
    .insert(database)
    .await
    .map_err(RotateRefreshError::Persistence)?;

    Ok(RotationOutcome::Rotated(IssuedSession {
        principal_id: family.principal_id,
        family_id: family.id,
        access_plaintext: access.plaintext(),
        refresh_plaintext: successor.plaintext(),
        access_expires_at,
        refresh_expires_at: family.absolute_expires_at,
    }))
}

async fn revoke_family_for_reuse<C>(
    database: &C,
    family: &session_family::Model,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), RotateRefreshError>
where
    C: sea_orm::ConnectionTrait,
{
    let mut family_model: session_family::ActiveModel = family.clone().into();
    family_model.revoked_at = Set(Some(now));
    family_model.revoked_reason = Set(Some(RevocationReason::RefreshReuse.as_str().to_owned()));
    family_model.updated_at = Set(now);
    family_model
        .update(database)
        .await
        .map_err(RotateRefreshError::Persistence)?;

    refresh_credential::Entity::update_many()
        .col_expr(
            refresh_credential::Column::RevokedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .filter(refresh_credential::Column::FamilyId.eq(family.id))
        .filter(refresh_credential::Column::RevokedAt.is_null())
        .exec(database)
        .await
        .map_err(RotateRefreshError::Persistence)?;

    security_audit_intent::ActiveModel {
        id: Set(uuid::Uuid::now_v7()),
        principal_id: Set(family.principal_id),
        family_id: Set(family.id),
        action: Set(actions::REUSE.to_owned()),
        state: Set(actions::PENDING.to_owned()),
        created_at: Set(now),
    }
    .insert(database)
    .await
    .map_err(RotateRefreshError::Persistence)?;

    Ok(())
}
