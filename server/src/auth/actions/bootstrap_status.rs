use sea_orm::{DatabaseConnection, DbErr, EntityTrait};

use crate::auth::entities::bootstrap_state;

/// Whether the owner exists. Unauthenticated on purpose: it reveals only
/// "setup done", which the sign-in screen needs to choose its form.
pub async fn execute(database: &DatabaseConnection) -> Result<bool, DbErr> {
    Ok(bootstrap_state::Entity::find_by_id(1)
        .one(database)
        .await?
        .is_some_and(|row| row.owner_principal_id.is_some()))
}
