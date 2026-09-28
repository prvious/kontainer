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
            actions::containers::list_containers::list_containers,
            actions::containers::container_action::container_action,
            actions::containers::containers_bulk_action::containers_bulk_action,
            actions::images::list_images::list_images,
            actions::images::remove_image::remove_image,
            actions::images::pull_image::pull_image,
            actions::nodes::list_nodes::list_nodes,
            actions::nodes::get_cluster_samples::get_cluster_samples,
            actions::nodes::get_node_samples::get_node_samples,
            actions::settings::get_settings::get_settings,
            actions::settings::update_settings::update_settings,
            actions::operations::get_operation::get_operation,
            actions::operations::list_operation_events::list_operation_events,
            actions::operations::cancel_operation::cancel_operation,
            actions::app::close_splashscreen::close_splashscreen,
            actions::app::load_app_settings::load_app_settings,
            actions::app::save_app_settings::save_app_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
