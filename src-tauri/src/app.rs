use crate::camera;
use crate::utils;
use crate::webgpu::{CameraSettingsUniform, WgpuState};
use crate::windows;
use nokhwa::Buffer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{sync::Arc, time::Instant};
use tauri::{async_runtime, Manager};

pub struct AppState {
    pub is_background_mode: AtomicBool,
    pub render_paused: AtomicBool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            is_background_mode: AtomicBool::new(false),
            render_paused: AtomicBool::new(false),
        }
    }
}

#[cfg(target_os = "macos")]
fn style_title_bar(main_webview_window: &tauri::WebviewWindow) {
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

pub fn app_setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_state = Arc::new(AppState::default());
    app.manage(app_state);

    let main_webview_window = app.get_webview_window("main").unwrap();
    style_title_bar(&main_webview_window);
    main_webview_window.show().unwrap();

    let main_window = app.get_window("main").unwrap();
    let overlay_window = windows::create_overlay_window(app.app_handle(), &main_window);

    let wgpu_state = async_runtime::block_on(WgpuState::new(overlay_window.clone()));
    app.manage(Arc::new(wgpu_state));

    // Create a channel for sending/receiving buffers from the camera
    let (tx, rx) = flume::unbounded::<Buffer>();
    async_runtime::spawn(async move {
        let mut camera = camera::create_camera();

        camera.open_stream().expect("Could not open stream");

        for _i in 0..1000 {
            let buffer = camera.frame().expect("Could not get frame");
            if tx.send(buffer).is_err() {
                break;
            }
        }

        camera.stop_stream().expect("Could not stop stream");
    });

    // Render loop
    let app_handle = app.app_handle().clone();
    async_runtime::spawn(async move {
        let wgpu_state = app_handle.state::<Arc<WgpuState>>();
        let app_state = app_handle.state::<Arc<AppState>>();

        while let Ok(buffer) = rx.recv() {
            let _t = Instant::now();

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

            let bytes = utils::yuyv_to_rgba(buffer.buffer(), width as usize, height as usize);

            // Calculate aspect-ratio-preserving camera settings
            let camera_aspect = width as f32 / height as f32;
            let config = wgpu_state.config.read().unwrap();
            let window_aspect = config.width as f32 / config.height as f32;
            drop(config);

            let (size_x, size_y) = if camera_aspect > window_aspect {
                // Camera is wider than window - fit to width
                (2.0, 2.0 / camera_aspect * window_aspect)
            } else {
                // Camera is taller than window - fit to height
                (2.0 * camera_aspect / window_aspect, 2.0)
            };

            let camera_settings = CameraSettingsUniform {
                position: [0.0, 0.0],
                size: [size_x, size_y],
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
            let bind_group = wgpu_state
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
                            resource: wgpu::BindingResource::Sampler(&wgpu_state.sampler),
                        },
                    ],
                    label: None,
                });

            // Attempt to get the surface texture
            let surface = wgpu_state.surface.read().unwrap();
            let output = match surface.get_current_texture() {
                Ok(output) => output,
                Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                    // Outdated surface, reconfigure it
                    let config = wgpu_state.config.read().unwrap();
                    surface.configure(&wgpu_state.device, &config);

                    match surface.get_current_texture() {
                        Ok(output) => output,
                        Err(_e) => {
                            continue;
                        }
                    }
                }
                Err(_e) => {
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
    });

    Ok(())
}
