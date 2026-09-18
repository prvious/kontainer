use chrono::Utc;
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter};

use crate::auth::{
    credentials::hash_presented,
    domain::{PrincipalStatus, ValidatedSession},
    entities::{principal, session_family},
};

#[derive(Debug, thiserror::Error)]
pub enum ValidateAccessError {
    #[error("access credential is not valid")]
    Unauthorized,
    #[error("access validation persistence failed")]
    Persistence(#[source] DbErr),
}

pub async fn execute(
    database: &DatabaseConnection,
    presented: &str,
) -> Result<ValidatedSession, ValidateAccessError> {
    // Every presented value follows the same hash-lookup path: malformed,
    // wrong-kind, unknown, expired, and revoked credentials are
    // indistinguishable without a database hit on the hash index.
    let hash = hash_presented(presented);
    let family = session_family::Entity::find()
        .filter(session_family::Column::AccessHash.eq(hash))
        .one(database)
        .await
        .map_err(ValidateAccessError::Persistence)?
        .ok_or(ValidateAccessError::Unauthorized)?;

    let now = Utc::now();
    if family.revoked_at.is_some()
        || family.access_expires_at <= now
        || family.absolute_expires_at <= now
    {
        return Err(ValidateAccessError::Unauthorized);
    }

    let owner = principal::Entity::find_by_id(family.principal_id)
        .one(database)
        .await
        .map_err(ValidateAccessError::Persistence)?
        .ok_or(ValidateAccessError::Unauthorized)?;
    if owner.status != PrincipalStatus::Active.as_str() {
        return Err(ValidateAccessError::Unauthorized);
    }

    Ok(ValidatedSession {
        principal_id: family.principal_id,
        family_id: family.id,
    })
}
