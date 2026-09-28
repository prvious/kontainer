pub mod get_settings;
pub mod update_settings;

/// Controller-wide settings as the API exposes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControllerSettings {
    pub metrics_retention_days: i32,
}
