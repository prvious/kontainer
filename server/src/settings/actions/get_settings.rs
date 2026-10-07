use sea_orm::{ConnectionTrait, DbErr, EntityTrait};

use super::ControllerSettings;
use crate::settings::entities::controller_setting;

/// Read the single settings row the migration seeds.
pub async fn execute(database: &impl ConnectionTrait) -> Result<ControllerSettings, DbErr> {
    let row = controller_setting::Entity::find_by_id(1_i16)
        .one(database)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("controller settings row is missing".to_owned()))?;
    Ok(ControllerSettings {
        metrics_retention_days: row.metrics_retention_days,
    })
}
