use crate::app;
use crate::webgpu;
use crate::windows_management;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{Manager, State};
use webgpu::WgpuState;

#[tauri::command]
pub fn toggle_camera_mode(
    app_handle: tauri::AppHandle,
    app_state: State<'_, Arc<app::AppState>>,
    wgpu_state: State<'_, Arc<WgpuState>>,
) -> bool {
    let current_mode = app_state.is_background_mode.load(Ordering::SeqCst);
    let new_mode_is_background = !current_mode;
    app_state
        .is_background_mode
        .store(new_mode_is_background, Ordering::SeqCst);

    // Pause rendering during surface switch to avoid race conditions
    app_state.render_paused.store(true, Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(10));
    // In background mode, the main window needs to be transparent
    // In thumbnail mode, the main window needs to be opaque
    windows_management::set_main_window_background_transparency(
        &app_handle,
        new_mode_is_background,
    );

    if new_mode_is_background {
        // 1 - Destroy overlay window
        // 2 - Switch wgpu surface
        // 3 - Restore focus to main window
        if let Some(overlay_window) = app_handle.get_window("camera-overlay") {
            let _ = overlay_window.close();
        }

        // Switch to main window and restore focus
        if let Some(main_window) = app_handle.get_window("main") {
            wgpu_state.switch_surface(main_window.clone());
            // This covers a weird edge case on MacOs where the Metal layer is not moved to the back
            // after the very first thumbnail-> background mode switch
            #[cfg(target_os = "macos")]
            windows_management::move_metal_layer_to_back(&app_handle);

            // Restore focus to main window after closing overlay
            let _ = main_window.set_focus();
        }
    } else {
        // Thumbnail mode
        // 1 - Clear main window surface
        // 2 - Create overlay window
        // 3 - Sync position and size of overlay window with main window
        // 4 - Switch wgpu surface to overlay window
        // 5 - Restore focus to main window
        if let Some(main_window) = app_handle.get_window("main") {
            windows_management::clear_main_window_surface(&wgpu_state);
            let overlay_window =
                windows_management::create_overlay_window(&app_handle, &main_window);
            if let Some(main_webview_window) = app_handle.get_webview_window("main") {
                windows_management::adjust_overlay_geometry(
                    &main_webview_window,
                    &overlay_window,
                    &wgpu_state,
                );
            }

            wgpu_state.switch_surface(overlay_window);

            let _ = main_window.set_focus();
        }
    }

    app_state.render_paused.store(false, Ordering::SeqCst);

    new_mode_is_background
}

#[tauri::command]
pub fn get_camera_mode(app_state: State<'_, Arc<app::AppState>>) -> bool {
    app_state.is_background_mode.load(Ordering::SeqCst)
}
