mod actions;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .invoke_handler(tauri::generate_handler![
            actions::auth::parse_auth_token::parse_auth_token,
            actions::app::close_splashscreen::close_splashscreen
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
