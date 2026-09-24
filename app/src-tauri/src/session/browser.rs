//! Native browser launch for the authorization URL.
//!
//! The authorization URL and its OAuth state never pass through WebView IPC:
//! Rust opens it directly after the controller returns it.

use tauri_plugin_opener::OpenerExt;

use super::errors::SessionError;

/// Opens an absolute URL in the operator's default browser.
pub(crate) trait BrowserOpener: Send + Sync + 'static {
    fn open(&self, url: &str) -> Result<(), SessionError>;
}

/// Production opener backed by `tauri-plugin-opener`.
pub(crate) struct PluginBrowserOpener {
    app: tauri::AppHandle,
}

impl PluginBrowserOpener {
    pub(crate) fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl BrowserOpener for PluginBrowserOpener {
    fn open(&self, url: &str) -> Result<(), SessionError> {
        self.app
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|_| SessionError::BrowserOpenFailed)
    }
}
