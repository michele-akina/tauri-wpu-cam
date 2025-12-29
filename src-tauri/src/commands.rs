use crate::app;
use crate::webgpu;
use crate::windows;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use webgpu::WgpuState;

fn set_main_window_background_transparency(app_handle: &AppHandle, new_mode_is_background: bool) {
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
fn move_metal_layer_to_back(app_handle: &AppHandle) {
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

fn clear_main_window_surface(wgpu_state: &WgpuState) {
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
    set_main_window_background_transparency(&app_handle, new_mode_is_background);

    // Switch the wgpu surface
    if new_mode_is_background {
        if let Some(overlay_window) = app_handle.get_window("camera-overlay") {
            let _ = overlay_window.close();
        }

        // Switch to main window and restore focus
        if let Some(main_window) = app_handle.get_window("main") {
            wgpu_state.switch_surface(main_window.clone());
            // This covers a weird edge case on MacOs where the Metal layer is not moved to the back
            // after the very first thumbnail-> background mode switch
            #[cfg(target_os = "macos")]
            move_metal_layer_to_back(&app_handle);

            // Restore focus to main window after closing overlay
            let _ = main_window.set_focus();
        }
    } else {
        // Thumbnail mode: recreate overlay window and switch to it
        if let Some(main_window) = app_handle.get_window("main") {
            clear_main_window_surface(&wgpu_state);
            let overlay_window = windows::create_overlay_window(&app_handle, &main_window);
            if let Some(main_webview_window) = app_handle.get_webview_window("main") {
                windows::sync_overlay_with_main(&main_webview_window, &overlay_window, &wgpu_state);
            }

            wgpu_state.switch_surface(overlay_window);

            // Restore focus to main window after creating overlay
            let _ = main_window.set_focus();
        }
    }

    // Resume rendering
    app_state.render_paused.store(false, Ordering::SeqCst);

    new_mode_is_background
}

#[tauri::command]
pub fn get_camera_mode(app_state: State<'_, Arc<app::AppState>>) -> bool {
    app_state.is_background_mode.load(Ordering::SeqCst)
}
