mod actions;
mod session;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            // Session custody must be initialized before any IPC command can
            // reach it: legacy plaintext is invalidated here, and a failure
            // keeps the app from starting.
            let manager = session::initialize(app.handle())?;
            app.manage(manager);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            actions::session::session_bootstrap::session_bootstrap,
            actions::session::session_login::session_login,
            actions::session::session_register::session_register,
            actions::session::session_restore::session_restore,
            actions::session::session_retry_commit::session_retry_commit,
            actions::session::session_logout::session_logout,
            actions::session::session_connections::session_connections,
            actions::session::session_forget_connection::session_forget_connection,
            actions::session::connection_probe::connection_probe,
            actions::session::controller_health::controller_health,
            actions::inventory::get_configured_node::get_configured_node,
            actions::inventory::list_container_inventory::list_container_inventory,
            actions::app::close_splashscreen::close_splashscreen,
            actions::app::load_app_settings::load_app_settings,
            actions::app::save_app_settings::save_app_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
