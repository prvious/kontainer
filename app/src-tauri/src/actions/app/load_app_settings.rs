use serde::{Deserialize, Serialize};
use tauri_plugin_store::StoreExt;

/// Non-secret desktop preferences. Unknown keys (such as the removed `font`
/// field) are ignored so older `settings.json` files keep loading.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub refresh_interval: Option<u32>,
    pub terminal_shell: Option<String>,
    pub terminal_font_size: Option<u32>,
    pub terminal_font_family: Option<String>,
    pub terminal_scrollback: Option<u32>,
    pub scheme: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            refresh_interval: Some(5000),
            terminal_shell: None,
            terminal_font_size: None,
            terminal_font_family: None,
            terminal_scrollback: None,
            scheme: None,
        }
    }
}

#[tauri::command]
pub async fn load_app_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    let store = app
        .store("settings.json")
        .map_err(|e| format!("store error: {e}"))?;

    let settings: AppSettings = store
        .get("appSettings")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::AppSettings;

    #[test]
    fn settings_written_by_older_versions_still_load() {
        let stored = serde_json::json!({
            "refreshInterval": 10000,
            "terminalShell": "/bin/zsh",
            "terminalFontSize": 14,
            "terminalFontFamily": null,
            "terminalScrollback": null,
            "font": "figtree",
            "scheme": "light"
        });
        let settings: AppSettings = serde_json::from_value(stored).unwrap();
        assert_eq!(settings.refresh_interval, Some(10000));
        assert_eq!(settings.scheme.as_deref(), Some("light"));
        let rewritten = serde_json::to_value(&settings).unwrap();
        assert!(rewritten.get("font").is_none());
    }
}
