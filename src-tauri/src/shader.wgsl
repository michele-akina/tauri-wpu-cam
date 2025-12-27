// Camera settings uniform buffer
struct CameraSettings {
    // Position of the camera quad center in NDC space (-1 to 1)
    // x: horizontal position (1.0 = right edge)
    // y: vertical position (1.0 = top edge)
    position: vec2<f32>,
    // Size of the camera quad in NDC space (0 to 2)
    // x: width, y: height
    size: vec2<f32>,
    // Corner radius in pixels (will be normalized by quad dimensions)
    corner_radius: f32,
    // Aspect ratio of the quad (width/height) for circular corners
    aspect_ratio: f32,
    // Padding to align to 16 bytes
    _padding: vec2<f32>,
};

@group(0) @binding(0) var my_texture: texture_2d<f32>;
@group(0) @binding(1) var my_sampler: sampler;
@group(1) @binding(0) var<uniform> camera_settings: CameraSettings;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) quad_uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    var unit_positions = array<vec2<f32>, 6>(
        vec2<f32>(-0.5, -0.5),
        vec2<f32>(0.5, -0.5),
        vec2<f32>(-0.5, 0.5),
        vec2<f32>(-0.5, 0.5),
        vec2<f32>(0.5, -0.5),
        vec2<f32>(0.5, 0.5)
    );

    var tex_coords = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0)
    );

    var quad_uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0)
    );

    let unit_pos = unit_positions[in_vertex_index];
    var scaled_pos = unit_pos * camera_settings.size;
    let final_pos = scaled_pos + camera_settings.position;

    out.position = vec4<f32>(final_pos.x, final_pos.y, 0.0, 1.0);
    out.tex_coords = tex_coords[in_vertex_index];
    out.quad_uv = quad_uvs[in_vertex_index];

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let camera_color = textureSample(my_texture, my_sampler, in.tex_coords);

    // Work in pixel-like coordinates where both dimensions use the same scale
    // Use the shorter edge as reference (1.0), scale the longer edge by aspect ratio
    let aspect = camera_settings.aspect_ratio;
    let px = in.quad_uv.x * aspect;
    let py = in.quad_uv.y;

    let width = aspect;
    let height = 1.0;
    let r = camera_settings.corner_radius;

    // Check if we're in a corner region and if so, check circular distance
    let in_left = px < r;
    let in_right = px > width - r;
    let in_bottom = py < r;
    let in_top = py > height - r;

    var inside = true;

    // Bottom-left corner
    if (in_left && in_bottom) {
        let dist = distance(vec2<f32>(px, py), vec2<f32>(r, r));
        inside = dist <= r;
    }
    // Bottom-right corner
    else if (in_right && in_bottom) {
        let dist = distance(vec2<f32>(px, py), vec2<f32>(width - r, r));
        inside = dist <= r;
    }
    // Top-left corner
    else if (in_left && in_top) {
        let dist = distance(vec2<f32>(px, py), vec2<f32>(r, height - r));
        inside = dist <= r;
    }
    // Top-right corner
    else if (in_right && in_top) {
        let dist = distance(vec2<f32>(px, py), vec2<f32>(width - r, height - r));
        inside = dist <= r;
    }

    if (!inside) {
        discard;
    }

    return camera_color;
}
