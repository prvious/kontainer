use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};
use tauri_plugin_store::StoreExt;

/// The ranges the Settings page offers (`app/src/components/settings/settings-form.ts`). The
/// WebView is the less-trusted side, so Rust clamps whatever it sends or the file holds.
const REFRESH_INTERVAL_MS: RangeInclusive<u32> = 2_000..=60_000;
const TERMINAL_FONT_SIZE: RangeInclusive<u32> = 10..=24;
const TERMINAL_SCROLLBACK: RangeInclusive<u32> = 1_000..=50_000;
/// The controller's and agent's limit on an exec shell (`server/src/exec/domain.rs`).
const MAX_SHELL_LEN: usize = 256;

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

/// The shell rule the controller and agent apply to an exec open: not empty, at most 256 bytes,
/// and no control characters. Not "absolute": `docker exec` resolves `bash` through `PATH`.
pub fn valid_shell(shell: &str) -> bool {
    !shell.is_empty() && shell.len() <= MAX_SHELL_LEN && !shell.chars().any(char::is_control)
}

fn clamp(value: Option<u32>, range: &RangeInclusive<u32>) -> Option<u32> {
    value.map(|value| value.clamp(*range.start(), *range.end()))
}

impl AppSettings {
    /// Every figure clamped into the range the Settings page offers, and a shell the controller
    /// would refuse dropped, so exec lets the node pick one.
    pub fn bounded(self) -> Self {
        Self {
            refresh_interval: clamp(self.refresh_interval, &REFRESH_INTERVAL_MS),
            terminal_shell: self.terminal_shell.filter(|shell| valid_shell(shell)),
            terminal_font_size: clamp(self.terminal_font_size, &TERMINAL_FONT_SIZE),
            terminal_scrollback: clamp(self.terminal_scrollback, &TERMINAL_SCROLLBACK),
            ..self
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

    Ok(settings.bounded())
}

#[cfg(test)]
mod tests {
    use super::{valid_shell, AppSettings};

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

    #[test]
    fn figures_are_clamped_into_the_ranges_the_settings_page_offers() {
        let low = AppSettings {
            refresh_interval: Some(1),
            terminal_font_size: Some(0),
            terminal_scrollback: Some(10),
            ..AppSettings::default()
        }
        .bounded();
        assert_eq!(low.refresh_interval, Some(2_000));
        assert_eq!(low.terminal_font_size, Some(10));
        assert_eq!(low.terminal_scrollback, Some(1_000));

        let high = AppSettings {
            refresh_interval: Some(u32::MAX),
            terminal_font_size: Some(400),
            terminal_scrollback: Some(10_000_000),
            ..AppSettings::default()
        }
        .bounded();
        assert_eq!(high.refresh_interval, Some(60_000));
        assert_eq!(high.terminal_font_size, Some(24));
        assert_eq!(high.terminal_scrollback, Some(50_000));

        let unset = AppSettings {
            refresh_interval: None,
            ..AppSettings::default()
        };
        assert_eq!(unset.bounded().refresh_interval, None);

        let within = || AppSettings {
            refresh_interval: Some(30_000),
            terminal_shell: Some("/bin/zsh".into()),
            terminal_font_size: Some(14),
            terminal_scrollback: Some(10_000),
            scheme: Some("dark".into()),
            ..AppSettings::default()
        };
        assert_eq!(within().bounded(), within());
    }

    #[test]
    fn shells_follow_the_controller_rule_and_a_refused_one_is_dropped() {
        assert!(valid_shell("/bin/ash"));
        assert!(
            valid_shell("bash"),
            "PATH lookup is fine, as in docker exec"
        );
        assert!(valid_shell(&"/".repeat(256)));
        assert!(!valid_shell(""));
        assert!(!valid_shell(&"/".repeat(257)));
        assert!(!valid_shell("/bin/sh\n"));
        assert!(!valid_shell("/bin/\u{7f}sh"));

        let refused = AppSettings {
            terminal_shell: Some("/bin/sh\0".into()),
            ..AppSettings::default()
        };
        assert_eq!(refused.bounded().terminal_shell, None);
    }
}
