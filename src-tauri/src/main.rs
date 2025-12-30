// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod camera;
mod commands;
mod webgpu;
mod windows_management;

fn main() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .setup(app::app_setup)
        .invoke_handler(tauri::generate_handler![
            commands::toggle_camera_mode,
            commands::get_camera_mode
        ])
        .build(tauri::generate_context!())
        .expect("Error while building tauri application")
        .run(windows_management::sync_camera_window_with_main);
}
