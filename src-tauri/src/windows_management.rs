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

    #[cfg(target_os = "macos")]
    {
        style_child_window(&overlay_window);
    }

    overlay_window.show().unwrap();

    // Re-apply size and position after show() to force compositor to display the window
    let _ = overlay_window.set_size(overlay_size);
    let _ = overlay_window.set_position(overlay_pos);

    overlay_window
}

#[cfg(target_os = "macos")]
fn style_child_window(window: &Window) {
    use objc2_app_kit::NSView;

    let _ = window.set_ignore_cursor_events(true);

    if let Ok(ns_view_ptr) = window.ns_view() {
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

pub fn sync_camera_window_with_main(app_handle: &AppHandle, event: RunEvent) {
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
                    adjust_overlay_geometry(&main_window, &overlay_window, &wgpu_state);
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
                    adjust_overlay_geometry(&main_window, &overlay_window, &wgpu_state);
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

pub fn adjust_overlay_geometry(
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

pub fn set_main_window_background_transparency(
    app_handle: &AppHandle,
    new_mode_is_background: bool,
) {
    if let Some(main_window) = app_handle.get_webview_window("main") {
        #[cfg(target_os = "macos")]
        {
            use objc2_app_kit::{NSColor, NSView};
            use objc2_foundation::MainThreadMarker;

            // Set the main window's background transparency
            if let Ok(ns_view_ptr) = main_window.ns_view() {
                unsafe {
                    let ns_view: &NSView = &*(ns_view_ptr as *const NSView);

                    if let Some(window) = ns_view.window() {
                        if new_mode_is_background {
                            // Make main window transparent
                            window.setOpaque(false);
                            if let Some(_mtm) = MainThreadMarker::new() {
                                window.setBackgroundColor(Some(&NSColor::clearColor()));
                            }
                        } else {
                            // Thumbnail mode: make main window opaque
                            window.setOpaque(true);
                            if let Some(_mtm) = MainThreadMarker::new() {
                                window.setBackgroundColor(Some(&NSColor::windowBackgroundColor()));
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub fn move_metal_layer_to_back(app_handle: &AppHandle) {
    use objc2_app_kit::NSView;
    use objc2_foundation::NSArray;

    if let Some(main_webview_window) = app_handle.get_webview_window("main") {
        if let Ok(ns_view_ptr) = main_webview_window.ns_view() {
            unsafe {
                let ns_view: &NSView = &*(ns_view_ptr as *const NSView);
                if let Some(layer) = ns_view.layer() {
                    if let Some(sublayers) = layer.sublayers() {
                        // Find the Metal layer (CAMetalLayer) and move it to the back
                        let count = sublayers.len();
                        if count >= 2 {
                            let metal_layer = NSArray::objectAtIndex(&sublayers, count - 1);
                            metal_layer.setZPosition(-1.0);
                        }
                    }
                }
            }
        }
    }
}

pub fn clear_main_window_surface(wgpu_state: &WgpuState) {
    let surface = wgpu_state.surface.read().unwrap();
    if let Ok(output) = surface.get_current_texture() {
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = wgpu_state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        wgpu_state.queue.submit(Some(encoder.finish()));
        output.present();
    }
}

#[cfg(target_os = "macos")]
pub fn style_title_bar(main_webview_window: &tauri::WebviewWindow) {
    use objc2_app_kit::NSView;

    if let Ok(ns_view_ptr) = main_webview_window.ns_view() {
        unsafe {
            let ns_view: &NSView = &*(ns_view_ptr as *const NSView);
            if let Some(window) = ns_view.window() {
                window.setTitlebarAppearsTransparent(true);
            }
        }
    }
}
