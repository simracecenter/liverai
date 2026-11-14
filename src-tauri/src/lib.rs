//! Livery AI core: the Rust side of the Tauri app.

pub mod cars;
pub mod claude;
pub mod commands;
pub mod error;
pub mod fsutil;
pub mod projects;
pub mod settings;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::settings_get,
            commands::settings_set,
            commands::cars_list,
            commands::car_get,
            commands::projects_list,
            commands::project_create,
            commands::project_get,
            commands::claude_status,
            commands::claude_open_signin,
            commands::claude_test,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Livery AI");
}
