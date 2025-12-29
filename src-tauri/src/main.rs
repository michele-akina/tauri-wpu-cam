// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod camera;
mod commands;
mod utils;
mod webgpu;
mod windows;

fn main() {
    tauri::Builder::default()
        .setup(app::app_setup)
        .invoke_handler(tauri::generate_handler![
            commands::toggle_camera_mode,
            commands::get_camera_mode
        ])
        .build(tauri::generate_context!())
        .expect("Error while building tauri application")
        .run(windows::sync_windows);
}
