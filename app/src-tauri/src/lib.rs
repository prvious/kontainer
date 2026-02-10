mod actions;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            actions::auth::auth_listen::auth_listen,
            actions::auth::parse_auth_token::parse_auth_token,
            actions::auth::save_session::save_session,
            actions::auth::load_session::load_session,
            actions::auth::delete_session::delete_session,
            actions::auth::load_profiles::load_profiles,
            actions::auth::save_profile::save_profile,
            actions::auth::delete_profile::delete_profile,
            actions::auth::set_active_profile::set_active_profile,
            actions::agent::fetch_providers::fetch_providers,
            actions::agent::ping_agent::ping_agent,
            actions::agent::list_services::list_services,
            actions::agent::inspect_service::inspect_service,
            actions::agent::list_service_tasks::list_service_tasks,
            actions::agent::inspect_task::inspect_task,
            actions::agent::list_stacks::list_stacks,
            actions::agent::remove_stack::remove_stack,
            actions::agent::list_networks::list_networks,
            actions::agent::list_volumes::list_volumes,
            actions::agent::list_nodes::list_nodes,
            actions::agent::list_task_stats::list_task_stats,
            actions::agent::scale_service::scale_service,
            actions::agent::restart_service::restart_service,
            actions::agent::rollback_service::rollback_service,
            actions::agent::update_service_env::update_service_env,
            actions::agent::update_service_image::update_service_image,
            actions::agent::update_node_availability::update_node_availability,
            actions::agent::update_node_role::update_node_role,
            actions::agent::list_secrets::list_secrets,
            actions::agent::create_secret::create_secret,
            actions::agent::remove_secret::remove_secret,
            actions::agent::list_configs::list_configs,
            actions::agent::create_config::create_config,
            actions::agent::remove_config::remove_config,
            actions::agent::inspect_config::inspect_config,
            actions::agent::list_cluster_metrics::list_cluster_metrics,
            actions::agent::list_node_metrics::list_node_metrics,
            actions::agent::list_images::list_images,
            actions::agent::remove_image::remove_image,
            actions::agent::prune_images::prune_images,
            actions::agent::get_agent_settings::get_agent_settings,
            actions::agent::update_agent_settings::update_agent_settings,
            actions::app::close_splashscreen::close_splashscreen,
            actions::app::load_app_settings::load_app_settings,
            actions::app::save_app_settings::save_app_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
