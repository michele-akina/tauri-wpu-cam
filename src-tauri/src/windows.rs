use crate::webgpu::WgpuState;
use std::sync::Arc;
use tauri::window::WindowBuilder;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, RunEvent, Window, WindowEvent};

const CAMERA_SIZE_FRACTION: f32 = 0.4;
const CAMERA_MARGIN_PX: i32 = 20;
const CAMERA_CORNER_RADIUS_PX: f64 = 12.0;

pub fn calculate_overlay_geometry(
    main_outer_pos: PhysicalPosition<i32>,
    main_inner_size: PhysicalSize<u32>,
    camera_aspect: f32,
) -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
    let overlay_width = (main_inner_size.width as f32 * CAMERA_SIZE_FRACTION) as u32;
    let overlay_height = (overlay_width as f32 / camera_aspect) as u32;

    let overlay_x =
        main_outer_pos.x + main_inner_size.width as i32 - overlay_width as i32 - CAMERA_MARGIN_PX;
    let overlay_y = main_outer_pos.y + CAMERA_MARGIN_PX;

    (
        PhysicalPosition::new(overlay_x, overlay_y),
        PhysicalSize::new(overlay_width.max(1), overlay_height.max(1)),
    )
}

pub fn create_overlay_window(app: &tauri::AppHandle, main_window: &Window) -> Window {
    let main_webview_window = app.get_webview_window("main").unwrap();
    let main_outer_pos = main_webview_window.outer_position().unwrap();
    let main_inner_size = main_webview_window.inner_size().unwrap();
    let (overlay_pos, overlay_size) =
        calculate_overlay_geometry(main_outer_pos, main_inner_size, 16.0 / 9.0);

    // create_child_window
    let overlay_window = WindowBuilder::new(app, "camera-overlay")
        .title("")
        .inner_size(overlay_size.width as f64, overlay_size.height as f64)
        .position(overlay_pos.x as f64, overlay_pos.y as f64)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .visible(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .parent(main_window)
        .expect("Failed to set parent window")
        .build()
        .expect("Failed to create overlay window");

    // Make overlay click-through, non-focusable, and set corner radius on macOS
    // style_child_window
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSView;

        let _ = overlay_window.set_ignore_cursor_events(true);

        if let Ok(ns_view_ptr) = overlay_window.ns_view() {
            unsafe {
                let ns_view: &NSView = &*(ns_view_ptr as *const NSView);
                ns_view.setWantsLayer(true);

                if let Some(layer) = ns_view.layer() {
                    layer.setCornerRadius(CAMERA_CORNER_RADIUS_PX);
                    layer.setMasksToBounds(true);
                    layer.setBorderWidth(0.0);
                }
            }
        }
    }

    overlay_window.show().unwrap();

    // Re-apply size and position after show() to force macOS compositor to display the window
    let _ = overlay_window.set_size(overlay_size);
    let _ = overlay_window.set_position(overlay_pos);

    overlay_window
}

pub fn sync_windows(app_handle: &AppHandle, event: RunEvent) {
    match event {
        RunEvent::WindowEvent {
            label,
            event: WindowEvent::Moved(_position),
            ..
        } if label == "main" => {
            // When main window moves, update overlay position
            if let Some(main_window) = app_handle.get_webview_window("main") {
                if let Some(overlay_window) = app_handle.get_window("camera-overlay") {
                    let wgpu_state = app_handle.state::<Arc<WgpuState>>();
                    sync_overlay_with_main(&main_window, &overlay_window, &wgpu_state);
                }
            }
        }
        RunEvent::WindowEvent {
            label,
            event: WindowEvent::Resized(_size),
            ..
        } if label == "main" => {
            // When main window resizes, update overlay size and position
            if let Some(main_window) = app_handle.get_webview_window("main") {
                if let Some(overlay_window) = app_handle.get_window("camera-overlay") {
                    let wgpu_state = app_handle.state::<Arc<WgpuState>>();
                    sync_overlay_with_main(&main_window, &overlay_window, &wgpu_state);
                }
            }
        }
        RunEvent::WindowEvent {
            label,
            event: WindowEvent::CloseRequested { .. },
            ..
        } if label == "main" => {
            // When main window closes, close overlay too
            if let Some(overlay_window) = app_handle.get_window("camera-overlay") {
                let _ = overlay_window.close();
            }
        }

        _ => (),
    }
}

pub fn sync_overlay_with_main(
    main_window: &tauri::WebviewWindow,
    overlay_window: &Window,
    wgpu_state: &Arc<WgpuState>,
) {
    if let (Ok(main_outer_pos), Ok(main_inner_size)) =
        (main_window.outer_position(), main_window.inner_size())
    {
        if let Ok(overlay_size) = overlay_window.inner_size() {
            // Preserve the current camera aspect ratio
            let camera_aspect = overlay_size.width as f32 / overlay_size.height.max(1) as f32;
            let (overlay_pos, new_overlay_size) =
                calculate_overlay_geometry(main_outer_pos, main_inner_size, camera_aspect);

            // Always update position (this is only called on window move/resize events, not every frame)
            let _ = overlay_window.set_position(overlay_pos);

            // Only resize if size changed
            if overlay_size.width != new_overlay_size.width
                || overlay_size.height != new_overlay_size.height
            {
                let _ = overlay_window.set_size(new_overlay_size);

                // Update wgpu surface config
                let mut config = wgpu_state.config.write().unwrap();
                config.width = new_overlay_size.width.max(1);
                config.height = new_overlay_size.height.max(1);
                drop(config);

                let mut needs_reconfigure = wgpu_state.needs_reconfigure.lock().unwrap();
                *needs_reconfigure = true;
                drop(needs_reconfigure);
            }
        }
    }
}
