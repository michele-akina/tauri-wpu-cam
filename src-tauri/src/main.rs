// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod utils;
mod webgpu;
use nokhwa::Buffer;
use std::{sync::Arc, time::Instant};
use tauri::{async_runtime, Manager, RunEvent, WindowEvent};
use webgpu::{CameraSettingsUniform, WgpuState};

// Camera overlay configuration constants
const CAMERA_SIZE_FRACTION: f32 = 0.4; // Camera takes up 30% of window width
const CAMERA_MARGIN: f32 = 0.05; // 5% margin from edges (in NDC, so 0.1 in -1 to 1 space)
const CAMERA_CORNER_RADIUS: f32 = 0.08; // Corner radius relative to quad size

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Get the main window
            let window = app.get_webview_window("main").unwrap();
            // Ensure window is visible and fully initialized (important for macOS Metal layer)
            window.show().unwrap();

            // Give macOS time to initialize the Metal layer to avoid null pointer dereference
            std::thread::sleep(std::time::Duration::from_millis(200));

            // Create a WgpuState (containing the device, instance, adapter etc.)
            // And store it in the state
            let wgpu_state = async_runtime::block_on(WgpuState::new(window));
            app.manage(Arc::new(wgpu_state));

            // Create a channel for sending/receiving buffers from the camera
            let (tx, rx) = std::sync::mpsc::channel::<Buffer>();

            let app_handle = app.app_handle().clone();

            // Spawn a thread for the camera
            async_runtime::spawn(async move {
                let mut camera = utils::create_camera();

                camera.open_stream().expect("Could not open stream");

                std::thread::sleep(std::time::Duration::from_secs(1));

                for i in 0..1000 {
                    let buffer = camera.frame().expect("Could not get frame");
                    if tx.send(buffer).is_err() {
                        println!("Render loop closed, stopping camera");
                        break;
                    }
                    println!("Frame {i} sent");
                }

                camera.stop_stream().expect("Could not stop stream");
                println!("Camera Stream Stopped");
            });

            async_runtime::spawn(async move {
                let wgpu_state = app_handle.state::<Arc<WgpuState>>();

                while let Ok(buffer) = rx.recv() {
                    // Check if we need to reconfigure the surface
                    {
                        let mut needs_reconfigure = wgpu_state.needs_reconfigure.lock().unwrap();
                        if *needs_reconfigure {
                            let config = wgpu_state.config.read().unwrap();
                            wgpu_state.surface.configure(&wgpu_state.device, &config);
                            *needs_reconfigure = false;
                            println!("Surface reconfigured to {}x{}", config.width, config.height);
                            drop(config);
                        }
                        drop(needs_reconfigure);
                    }

                    let t = Instant::now();
                    let width = buffer.resolution().width();
                    let height = buffer.resolution().height();
                    let bytes =
                        utils::yuyv_to_rgba(buffer.buffer(), width as usize, height as usize);
                    println!("Decoding took: {}ms", t.elapsed().as_millis());

                    // Update camera settings based on current window size
                    {
                        let config = wgpu_state.config.read().unwrap();
                        let window_width = config.width as f32;
                        let window_height = config.height as f32;
                        let window_aspect = window_width / window_height;

                        // Calculate camera quad size
                        // Width is a fraction of the window, height maintains camera aspect ratio
                        let camera_aspect = width as f32 / height as f32;
                        let quad_width_ndc = CAMERA_SIZE_FRACTION * 2.0; // Convert to NDC space (-1 to 1)
                        let quad_height_ndc = quad_width_ndc * window_aspect / camera_aspect;

                        // Position in top-right corner with margin
                        // NDC goes from -1 (left/bottom) to 1 (right/top)
                        let margin_ndc = CAMERA_MARGIN * 2.0;
                        let pos_x = 1.0 - margin_ndc - quad_width_ndc / 2.0;
                        // Multiply Y margin by window_aspect to get same pixel distance as X margin
                        let margin_ndc_y = margin_ndc * window_aspect;
                        let pos_y = 1.0 - margin_ndc_y - quad_height_ndc / 2.0;

                        // Calculate aspect ratio for the corner radius calculation
                        let quad_aspect = quad_width_ndc / quad_height_ndc;

                        let camera_settings = CameraSettingsUniform {
                            position: [pos_x, pos_y],
                            size: [quad_width_ndc, quad_height_ndc],
                            corner_radius: CAMERA_CORNER_RADIUS,
                            aspect_ratio: quad_aspect,
                            _padding: [0.0, 0.0],
                        };

                        wgpu_state.update_camera_settings(&camera_settings);
                        drop(config);
                    }

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
                    let output = match wgpu_state.surface.get_current_texture() {
                        Ok(output) => {
                            println!("Successfully acquired surface texture");
                            output
                        }
                        Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                            println!("Surface outdated or lost, reconfiguring...");

                            let config = wgpu_state.config.read().unwrap();
                            wgpu_state.surface.configure(&wgpu_state.device, &config);
                            drop(config);

                            match wgpu_state.surface.get_current_texture() {
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
                                    // Clear to transparent black to show the webview behind
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

                    println!("Frame rendered in: {}ms", t.elapsed().as_millis());
                }

                println!("Render loop ended");
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            match event {
                RunEvent::WebviewEvent { label, event, .. } => {
                    println!("Received event from {}: {:?}", label, event);
                }
                RunEvent::WindowEvent {
                    label: _,
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    let wgpu_state = app_handle.state::<Arc<WgpuState>>();

                    // Update the config and mark that we need to reconfigure
                    let mut config = wgpu_state.config.write().unwrap();
                    config.width = size.width.max(1);
                    config.height = size.height.max(1);
                    drop(config);

                    // Set the flag so the render loop will reconfigure on the next frame
                    let mut needs_reconfigure = wgpu_state.needs_reconfigure.lock().unwrap();
                    *needs_reconfigure = true;
                    drop(needs_reconfigure);

                    println!("Resize requested to {}x{}", size.width, size.height);
                }
                _ => (),
            }
        });
}
