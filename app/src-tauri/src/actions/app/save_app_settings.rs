use tauri_plugin_store::StoreExt;

use super::load_app_settings::{valid_shell, AppSettings};

/// The settings as stored: a shell the controller would refuse is rejected, figures are clamped.
fn checked(settings: AppSettings) -> Result<AppSettings, String> {
    if settings
        .terminal_shell
        .as_deref()
        .is_some_and(|shell| !valid_shell(shell))
    {
        return Err("invalid_shell".into());
    }
    Ok(settings.bounded())
}

#[tauri::command]
pub async fn save_app_settings(app: tauri::AppHandle, settings: AppSettings) -> Result<(), String> {
    let settings = checked(settings)?;
    let store = app
        .store("settings.json")
        .map_err(|e| format!("store error: {e}"))?;

    store.set(
        "appSettings",
        serde_json::to_value(&settings).map_err(|e| format!("serialize error: {e}"))?,
    );
    store.save().map_err(|e| format!("store save error: {e}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{checked, AppSettings};

    #[test]
    fn a_refused_shell_is_not_saved_and_figures_are_clamped() {
        for shell in ["", "/bin/sh\r", "x\u{1b}[2J"] {
            let settings = AppSettings {
                terminal_shell: Some(shell.into()),
                ..AppSettings::default()
            };
            assert_eq!(checked(settings), Err("invalid_shell".into()), "{shell:?}");
        }

        let saved = checked(AppSettings {
            refresh_interval: Some(1),
            terminal_shell: Some("bash".into()),
            terminal_font_size: Some(99),
            ..AppSettings::default()
        })
        .unwrap();
        assert_eq!(saved.refresh_interval, Some(2_000));
        assert_eq!(saved.terminal_shell.as_deref(), Some("bash"));
        assert_eq!(saved.terminal_font_size, Some(24));
    }
}
