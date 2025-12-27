use std::sync::Mutex;
use std::sync::RwLock;
use tauri::WebviewWindow;
use wgpu::util::DeviceExt;

/// Camera settings uniform buffer data
/// Must match the WGSL struct layout exactly
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraSettingsUniform {
    /// Position of the camera quad center in NDC space (-1 to 1)
    pub position: [f32; 2],
    /// Size of the camera quad in NDC space (0 to 2)
    pub size: [f32; 2],
    /// Corner radius in normalized quad space (0 to 0.5)
    pub corner_radius: f32,
    /// Aspect ratio of the quad (width/height) for circular corners
    pub aspect_ratio: f32,
    /// Padding to align to 16 bytes
    pub _padding: [f32; 2],
}

impl Default for CameraSettingsUniform {
    fn default() -> Self {
        Self {
            // Top-right corner position (center of the camera quad)
            position: [0.65, 0.65],
            // Size in NDC (width=0.6, height=0.6 of the screen)
            size: [0.6, 0.6],
            // Corner radius (relative to quad size, 0.1 = nice rounded corners)
            corner_radius: 0.08,
            // Will be calculated based on actual dimensions
            aspect_ratio: 1.0,
            _padding: [0.0, 0.0],
        }
    }
}

pub struct WgpuState<'win> {
    pub queue: wgpu::Queue,
    pub device: wgpu::Device,
    pub sampler: wgpu::Sampler,
    pub surface: wgpu::Surface<'win>,
    pub render_pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub config: RwLock<wgpu::SurfaceConfiguration>,
    pub needs_reconfigure: Mutex<bool>,
    // Camera settings uniform
    pub camera_settings_buffer: wgpu::Buffer,
    pub camera_settings_bind_group: wgpu::BindGroup,
}

impl WgpuState<'_> {
    pub async fn new(window: WebviewWindow) -> Self {
        let size = window.inner_size().unwrap();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .expect("Failed to create device");

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        // Bind group layout for texture and sampler
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("texture_bind_group_layout"),
        });

        // Bind group layout for camera settings uniform
        let camera_settings_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_settings_bind_group_layout"),
            });

        // Create camera settings uniform buffer with default values
        let camera_settings = CameraSettingsUniform::default();
        let camera_settings_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Settings Buffer"),
            contents: bytemuck::cast_slice(&[camera_settings]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_settings_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_settings_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_settings_buffer.as_entire_binding(),
            }],
            label: Some("camera_settings_bind_group"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout, &camera_settings_bind_group_layout],
            immediate_size: 0,
        });

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: swapchain_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            multisample: wgpu::MultisampleState::default(),
            depth_stencil: None,
            multiview_mask: None,
            cache: None,
        });

        let config = wgpu::SurfaceConfiguration {
            width: size.width.max(1),
            height: size.height.max(1),
            format: swapchain_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        Self {
            device,
            queue,
            surface,
            render_pipeline,
            config: RwLock::new(config),
            sampler,
            bind_group_layout,
            needs_reconfigure: Mutex::new(false),
            camera_settings_buffer,
            camera_settings_bind_group,
        }
    }

    /// Update camera settings uniform buffer
    pub fn update_camera_settings(&self, settings: &CameraSettingsUniform) {
        self.queue.write_buffer(
            &self.camera_settings_buffer,
            0,
            bytemuck::cast_slice(&[*settings]),
        );
    }
}
