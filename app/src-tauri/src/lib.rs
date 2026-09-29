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
            app.manage(session::log_streams::LogStreams::default());
            app.manage(session::exec_sessions::ExecSessions::default());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Log streams and shells belong to the window that opened them.
            if let tauri::WindowEvent::Destroyed = event {
                window
                    .state::<session::log_streams::LogStreams>()
                    .cancel_all();
                window
                    .state::<session::exec_sessions::ExecSessions>()
                    .cancel_all();
            }
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
            actions::volumes::list_volumes::list_volumes,
            actions::volumes::create_volume::create_volume,
            actions::volumes::remove_volume::remove_volume,
            actions::networks::list_networks::list_networks,
            actions::networks::inspect_network::inspect_network,
            actions::networks::create_network::create_network,
            actions::networks::remove_network::remove_network,
            actions::services::list_services::list_services,
            actions::services::get_service::get_service,
            actions::services::scale_service::scale_service,
            actions::services::restart_service::restart_service,
            actions::services::rollback_service::rollback_service,
            actions::stacks::list_stacks::list_stacks,
            actions::stacks::get_stack_removal_plan::get_stack_removal_plan,
            actions::stacks::remove_stack::remove_stack,
            actions::nodes::list_nodes::list_nodes,
            actions::logs::logs_open::logs_open,
            actions::logs::logs_close::logs_close,
            actions::logs::logs_retry_now::logs_retry_now,
            actions::exec::exec_open::exec_open,
            actions::exec::exec_write::exec_write,
            actions::exec::exec_resize::exec_resize,
            actions::exec::exec_close::exec_close,
            actions::nodes::get_cluster_summary::get_cluster_summary,
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
