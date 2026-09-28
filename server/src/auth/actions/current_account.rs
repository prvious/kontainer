use sea_orm::{DatabaseConnection, DbErr, EntityTrait};
use uuid::Uuid;

use crate::auth::entities::principal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub principal_id: Uuid,
    pub email: Option<String>,
    pub display_name: Option<String>,
}

/// The signed-in principal's non-secret identity for the titlebar and settings.
pub async fn execute(
    database: &DatabaseConnection,
    principal_id: Uuid,
) -> Result<Option<Account>, DbErr> {
    Ok(principal::Entity::find_by_id(principal_id)
        .one(database)
        .await?
        .map(|row| Account {
            principal_id: row.id,
            email: row.email,
            display_name: row.display_name,
        }))
}
