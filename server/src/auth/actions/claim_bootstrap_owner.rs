//! The singleton bootstrap claim shared by every first-owner path.
//!
//! GitHub redemption and password registration both lock `bootstrap_state`
//! row 1, create the active owner principal, and claim the row in the same
//! transaction, so exactly one caller ever observes the unclaimed state.

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DbErr, EntityTrait, QuerySelect,
    sea_query::LockType,
};
use uuid::Uuid;

use crate::auth::{
    domain::PrincipalStatus,
    entities::{bootstrap_state, principal},
};

/// Lock the singleton row. `None` means the migration invariant is broken.
pub(crate) async fn lock_singleton<C: ConnectionTrait>(
    database: &C,
) -> Result<Option<bootstrap_state::Model>, DbErr> {
    bootstrap_state::Entity::find_by_id(1)
        .lock(LockType::Update)
        .one(database)
        .await
}

/// Insert the active human principal that becomes the owner.
pub(crate) async fn insert_owner_principal<C: ConnectionTrait>(
    database: &C,
    now: DateTime<Utc>,
    email: Option<String>,
    display_name: Option<String>,
) -> Result<Uuid, DbErr> {
    let principal_id = Uuid::now_v7();
    principal::ActiveModel {
        id: Set(principal_id),
        kind: Set("human".to_owned()),
        status: Set(PrincipalStatus::Active.as_str().to_owned()),
        authorization_epoch: Set(0),
        email: Set(email),
        display_name: Set(display_name),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(database)
    .await?;
    Ok(principal_id)
}

/// Point the locked singleton at its owner.
pub(crate) async fn claim<C: ConnectionTrait>(
    database: &C,
    bootstrap: &bootstrap_state::Model,
    principal_id: Uuid,
    now: DateTime<Utc>,
) -> Result<(), DbErr> {
    let mut claimed: bootstrap_state::ActiveModel = bootstrap.clone().into();
    claimed.owner_principal_id = Set(Some(principal_id));
    claimed.claimed_at = Set(Some(now));
    claimed.updated_at = Set(now);
    claimed.update(database).await?;
    Ok(())
}
