use sea_orm::{
    ActiveValue::{Set, Unchanged},
    DatabaseConnection, DbErr, EntityTrait,
};

use super::ControllerSettings;
use crate::{nodes::actions::database_now, settings::entities::controller_setting};

pub const RETENTION_DAYS: std::ops::RangeInclusive<i32> = 1..=365;

#[derive(Debug, thiserror::Error)]
pub enum UpdateSettingsError {
    #[error("metricsRetentionDays must be between 1 and 365")]
    RetentionOutOfRange,
    #[error("settings persistence failed")]
    Persistence(#[from] DbErr),
}

/// Fields the caller wants changed; `None` leaves a field as it is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SettingsPatch {
    pub metrics_retention_days: Option<i32>,
}

pub async fn execute(
    database: &DatabaseConnection,
    patch: SettingsPatch,
) -> Result<ControllerSettings, UpdateSettingsError> {
    if let Some(days) = patch.metrics_retention_days {
        if !RETENTION_DAYS.contains(&days) {
            return Err(UpdateSettingsError::RetentionOutOfRange);
        }
        let now = database_now(database).await?;
        controller_setting::Entity::update(controller_setting::ActiveModel {
            id: Unchanged(1),
            metrics_retention_days: Set(days),
            updated_at: Set(now),
        })
        .exec(database)
        .await?;
    }
    Ok(super::get_settings::execute(database).await?)
}
