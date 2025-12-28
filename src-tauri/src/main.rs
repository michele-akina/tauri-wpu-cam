// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod utils;
mod webgpu;

use nokhwa::Buffer;
use std::sync::atomic::{AtomicBool, Ordering};

use std::{sync::Arc, time::Instant};
use tauri::window::WindowBuilder;
use tauri::{
    async_runtime, Manager, PhysicalPosition, PhysicalSize, RunEvent, State, Window, WindowEvent,
};
use webgpu::{CameraSettingsUniform, WgpuState};

/// Render mode for the camera
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// Camera renders in a small overlay window (thumbnail in corner)
    Thumbnail,
    /// Camera renders fullscreen behind the main window's WebView
    Background,
}

/// Application state for managing render mode
pub struct AppState {
    /// Current render mode (true = Background, false = Thumbnail)
    pub is_background_mode: AtomicBool,
    /// Flag to pause rendering during surface switch
    pub render_paused: AtomicBool,
    /// Flag to indicate a switch is in progress (for debouncing)
    pub switch_in_progress: AtomicBool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            is_background_mode: AtomicBool::new(false), // Start in Thumbnail mode
            render_paused: AtomicBool::new(false),
            switch_in_progress: AtomicBool::new(false),
        }
    }
}

/// Toggle between Thumbnail and Background camera modes
#[tauri::command]
fn toggle_camera_mode(
    app_handle: tauri::AppHandle,
    app_state: State<'_, Arc<AppState>>,
    wgpu_state: State<'_, Arc<WgpuState>>,
) -> bool {
    // Debounce: ignore if a switch is already in progress
    if app_state.switch_in_progress.swap(true, Ordering::SeqCst) {
        return app_state.is_background_mode.load(Ordering::SeqCst);
    }

    // Toggle the mode
    let current = app_state.is_background_mode.load(Ordering::SeqCst);
    let new_mode = !current;
    app_state
        .is_background_mode
        .store(new_mode, Ordering::SeqCst);

    // Pause rendering during surface switch to avoid race conditions
    app_state.render_paused.store(true, Ordering::SeqCst);

    // Brief delay to ensure render loop has seen the pause flag
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Update window transparency BEFORE switching surface
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
                        if new_mode {
                            // Background mode: make window transparent
                            window.setOpaque(false);
                            if let Some(_mtm) = MainThreadMarker::new() {
                                window.setBackgroundColor(Some(&NSColor::clearColor()));
                            }
                        } else {
                            // Thumbnail mode: make window opaque
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

    // Switch the wgpu surface (must happen on main thread for Metal)
    if new_mode {
        // Background mode: close overlay first, then switch to main window
        // Close the overlay window
        if let Some(overlay_window) = app_handle.get_window("camera-overlay") {
            let _ = overlay_window.close();
        }

        // Switch to main window and restore focus
        if let Some(main_window) = app_handle.get_window("main") {
            println!("Switching to background mode (main window)");
            wgpu_state.switch_surface(main_window.clone());

            // Restore focus to main window after closing overlay
            let _ = main_window.set_focus();
        }
    } else {
        // Thumbnail mode: recreate overlay window and switch to it
        if let Some(main_window) = app_handle.get_window("main") {
            println!("Switching to thumbnail mode (overlay window)");

            // Clear the main window surface before switching away
            {
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

            let overlay_window = create_overlay_window(&app_handle, &main_window);

            // Sync position with main window
            if let Some(main_webview_window) = app_handle.get_webview_window("main") {
                sync_overlay_with_main(&main_webview_window, &overlay_window, &wgpu_state);
            }

            wgpu_state.switch_surface(overlay_window);

            // Restore focus to main window after creating overlay
            let _ = main_window.set_focus();
        }
    }

    // Resume rendering and allow new switches
    app_state.render_paused.store(false, Ordering::SeqCst);
    app_state.switch_in_progress.store(false, Ordering::SeqCst);

    // Return the new mode (true = Background, false = Thumbnail)
    new_mode
}

/// Get the current camera mode
#[tauri::command]
fn get_camera_mode(app_state: State<'_, Arc<AppState>>) -> bool {
    app_state.is_background_mode.load(Ordering::SeqCst)
}

// Camera overlay configuration constants
const CAMERA_SIZE_FRACTION: f32 = 0.4; // Camera takes up 40% of main window width
const CAMERA_MARGIN_PX: i32 = 20; // Margin from edges in pixels
const CAMERA_CORNER_RADIUS_PX: f64 = 12.0; // Corner radius in pixels for window styling

/// Calculate the overlay window size and position based on main window
fn calculate_overlay_geometry(
    main_outer_pos: PhysicalPosition<i32>,
    main_inner_size: PhysicalSize<u32>,
    camera_aspect: f32,
) -> (PhysicalPosition<i32>, PhysicalSize<u32>) {
    // Calculate overlay size based on main window width
    let overlay_width = (main_inner_size.width as f32 * CAMERA_SIZE_FRACTION) as u32;
    let overlay_height = (overlay_width as f32 / camera_aspect) as u32;

    // Position in top-right corner of main window
    let overlay_x =
        main_outer_pos.x + main_inner_size.width as i32 - overlay_width as i32 - CAMERA_MARGIN_PX;
    let overlay_y = main_outer_pos.y + CAMERA_MARGIN_PX;

    (
        PhysicalPosition::new(overlay_x, overlay_y),
        PhysicalSize::new(overlay_width.max(1), overlay_height.max(1)),
    )
}

/// Create the camera overlay window
fn create_overlay_window(app: &tauri::AppHandle, main_window: &Window) -> Window {
    let main_webview_window = app.get_webview_window("main").unwrap();
    let main_outer_pos = main_webview_window.outer_position().unwrap();
    let main_inner_size = main_webview_window.inner_size().unwrap();
    let (overlay_pos, overlay_size) =
        calculate_overlay_geometry(main_outer_pos, main_inner_size, 16.0 / 9.0);

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

/// Sync overlay window position and size with main window
fn sync_overlay_with_main(
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

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Initialize application state
            let app_state = Arc::new(AppState::default());
            app.manage(app_state);

            // Get the main window (as WebviewWindow for webview operations, and as Window for parenting)
            let main_webview_window = app.get_webview_window("main").unwrap();

            // Make the title bar transparent on macOS
            #[cfg(target_os = "macos")]
            {
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

            main_webview_window.show().unwrap();

            // Get the same window as a Window type for use as parent
            let main_window = app.get_window("main").unwrap();

            // Create the overlay window using the helper function
            let overlay_window = create_overlay_window(&app.app_handle(), &main_window);

            // Create WgpuState for the overlay window
            let wgpu_state = async_runtime::block_on(WgpuState::new(overlay_window.clone()));

            app.manage(Arc::new(wgpu_state));

            // Create a channel for sending/receiving buffers from the camera
            let (tx, rx) = std::sync::mpsc::channel::<Buffer>();
            let app_handle = app.app_handle().clone();

            // Spawn a thread for the camera
            async_runtime::spawn(async move {
                let mut camera = utils::create_camera();

                camera.open_stream().expect("Could not open stream");

                for _i in 0..1000 {
                    let buffer = camera.frame().expect("Could not get frame");
                    if tx.send(buffer).is_err() {
                        println!("Render loop closed, stopping camera");
                        break;
                    }
                }

                camera.stop_stream().expect("Could not stop stream");
                println!("Camera Stream Stopped");
            });

            // Render loop
            async_runtime::spawn(async move {
                let wgpu_state = app_handle.state::<Arc<WgpuState>>();
                let app_state = app_handle.state::<Arc<AppState>>();

                while let Ok(buffer) = rx.recv() {
                    let _t = Instant::now();

                    // Skip rendering if paused (during surface switch)
                    if app_state.render_paused.load(Ordering::SeqCst) {
                        continue;
                    }

                    // Check if we need to reconfigure the surface
                    {
                        let mut needs_reconfigure = wgpu_state.needs_reconfigure.lock().unwrap();
                        if *needs_reconfigure {
                            let config = wgpu_state.config.read().unwrap();
                            let surface = wgpu_state.surface.read().unwrap();
                            surface.configure(&wgpu_state.device, &config);
                            *needs_reconfigure = false;
                        }
                    }

                    let width = buffer.resolution().width();
                    let height = buffer.resolution().height();
                    let bytes =
                        utils::yuyv_to_rgba(buffer.buffer(), width as usize, height as usize);

                    // Calculate aspect-ratio-preserving camera settings
                    let camera_aspect = width as f32 / height as f32;
                    let config = wgpu_state.config.read().unwrap();
                    let window_aspect = config.width as f32 / config.height as f32;
                    drop(config);

                    // Calculate size that maintains camera aspect ratio
                    let (size_x, size_y) = if camera_aspect > window_aspect {
                        // Camera is wider than window - fit to width, letterbox top/bottom
                        (2.0, 2.0 / camera_aspect * window_aspect)
                    } else {
                        // Camera is taller than window - fit to height, pillarbox left/right
                        (2.0 * camera_aspect / window_aspect, 2.0)
                    };

                    let camera_settings = CameraSettingsUniform {
                        position: [0.0, 0.0],   // Centered
                        size: [size_x, size_y], // Maintain aspect ratio
                        _padding: [0.0, 0.0, 0.0, 0.0],
                    };
                    wgpu_state.update_camera_settings(&camera_settings);

                    let texture_size = wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    };

                    let texture = wgpu_state.device.create_texture(&wgpu::TextureDescriptor {
                        label: None,
                        sample_count: 1,
                        mip_level_count: 1,
                        size: texture_size,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });

                    wgpu_state.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &bytes,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * width),
                            rows_per_image: Some(height),
                        },
                        texture_size,
                    );

                    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let bind_group =
                        wgpu_state
                            .device
                            .create_bind_group(&wgpu::BindGroupDescriptor {
                                layout: &wgpu_state.bind_group_layout,
                                entries: &[
                                    wgpu::BindGroupEntry {
                                        binding: 0,
                                        resource: wgpu::BindingResource::TextureView(&texture_view),
                                    },
                                    wgpu::BindGroupEntry {
                                        binding: 1,
                                        resource: wgpu::BindingResource::Sampler(
                                            &wgpu_state.sampler,
                                        ),
                                    },
                                ],
                                label: None,
                            });

                    // Attempt to get the surface texture
                    let surface = wgpu_state.surface.read().unwrap();
                    let output = match surface.get_current_texture() {
                        Ok(output) => output,
                        Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                            println!("Surface outdated or lost, reconfiguring...");

                            let config = wgpu_state.config.read().unwrap();
                            surface.configure(&wgpu_state.device, &config);

                            match surface.get_current_texture() {
                                Ok(output) => output,
                                Err(e) => {
                                    eprintln!("Failed to acquire texture after reconfigure: {}", e);
                                    continue;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to acquire next swap chain texture: {}", e);
                            continue;
                        }
                    };
                    drop(surface);

                    let view = output
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());

                    let mut encoder = wgpu_state
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                    {
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    // Clear to transparent so main window shows through
                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                        r: 0.0,
                                        g: 0.0,
                                        b: 0.0,
                                        a: 0.0,
                                    }),
                                    store: wgpu::StoreOp::Store,
                                },
                                depth_slice: None,
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                            multiview_mask: None,
                        });
                        rpass.set_pipeline(&wgpu_state.render_pipeline);
                        rpass.set_bind_group(0, &bind_group, &[]);
                        rpass.set_bind_group(1, &wgpu_state.camera_settings_bind_group, &[]);
                        rpass.draw(0..6, 0..1);
                    }

                    wgpu_state.queue.submit(Some(encoder.finish()));
                    output.present();
                }

                println!("Render loop ended");
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            toggle_camera_mode,
            get_camera_mode
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
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
        });
}
