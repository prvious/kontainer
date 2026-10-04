mod actions;
mod session;

use tauri::{webview::PageLoadEvent, Manager, Runtime};

/// Log streams and shells belong to the page that opened them: a window close or a reload
/// leaves nobody reading them, so they end and free their server slots.
fn end_page_streams<R: Runtime>(app: &impl Manager<R>) {
    if let Some(streams) = app.try_state::<session::log_streams::LogStreams>() {
        streams.cancel_all();
    }
    if let Some(shells) = app.try_state::<session::exec_sessions::ExecSessions>() {
        shells.cancel_all();
    }
}

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
            if let tauri::WindowEvent::Destroyed = event {
                end_page_streams(window);
            }
        })
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Started {
                end_page_streams(webview);
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
            actions::settings::get_status::get_status,
            actions::settings::update_settings::update_settings,
            actions::operations::get_operation::get_operation,
            actions::operations::cancel_operation::cancel_operation,
            actions::app::close_splashscreen::close_splashscreen,
            actions::app::load_app_settings::load_app_settings,
            actions::app::save_app_settings::save_app_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    /// Rust reads and writes `session.json` and `settings.json` through `StoreExt`. The WebView
    /// gets no store permission, so page script can't rewrite either file.
    #[test]
    fn the_webview_has_no_store_permission() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let permissions = capability["permissions"].as_array().unwrap();
        assert!(!permissions.is_empty());
        assert!(permissions.iter().all(|permission| !permission
            .as_str()
            .unwrap_or_default()
            .starts_with("store:")));
    }
}
