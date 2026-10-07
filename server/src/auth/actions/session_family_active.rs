use chrono::Utc;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait};
use uuid::Uuid;

use crate::auth::{
    domain::PrincipalStatus,
    entities::{principal, session_family},
};

/// Whether a session family may keep a long-lived stream open: not revoked, inside its absolute
/// lifetime, and owned by an active principal. Unlike `validate_access`, the access credential's
/// own five-minute expiry doesn't matter here; the desktop refreshes that independently.
pub async fn execute(database: &DatabaseConnection, family_id: Uuid) -> Result<bool, DbErr> {
    let Some(family) = session_family::Entity::find_by_id(family_id)
        .one(database)
        .await?
    else {
        return Ok(false);
    };
    if family.revoked_at.is_some() || family.absolute_expires_at <= Utc::now() {
        return Ok(false);
    }
    let owner = principal::Entity::find_by_id(family.principal_id)
        .one(database)
        .await?;
    Ok(owner.is_some_and(|owner| owner.status == PrincipalStatus::Active.as_str()))
}
