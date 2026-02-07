mod actions;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            actions::auth::parse_auth_token::parse_auth_token,
            actions::auth::save_session::save_session,
            actions::auth::load_session::load_session,
            actions::auth::delete_session::delete_session,
            actions::agent::ping_agent::ping_agent,
            actions::agent::list_services::list_services,
            actions::agent::inspect_service::inspect_service,
            actions::agent::list_service_tasks::list_service_tasks,
            actions::agent::inspect_task::inspect_task,
            actions::agent::list_stacks::list_stacks,
            actions::agent::remove_stack::remove_stack,
            actions::agent::list_nodes::list_nodes,
            actions::agent::list_task_stats::list_task_stats,
            actions::agent::scale_service::scale_service,
            actions::agent::restart_service::restart_service,
            actions::agent::rollback_service::rollback_service,
            actions::app::close_splashscreen::close_splashscreen
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
