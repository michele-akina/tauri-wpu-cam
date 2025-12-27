// Camera settings uniform buffer
struct CameraSettings {
    // Position of the camera quad center in NDC space (-1 to 1)
    // x: horizontal position (1.0 = right edge)
    // y: vertical position (1.0 = top edge)
    position: vec2<f32>,
    // Size of the camera quad in NDC space (0 to 2)
    // x: width, y: height
    size: vec2<f32>,
    // Corner radius in normalized quad space (0 to 0.5)
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
    @location(1) quad_uv: vec2<f32>,  // UV coordinates within the quad for rounded corners
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Unit quad vertices (two triangles) centered at origin, ranging from -0.5 to 0.5
    var unit_positions = array<vec2<f32>, 6>(
        vec2<f32>(-0.5, -0.5),  // Bottom-left
        vec2<f32>(0.5, -0.5),   // Bottom-right
        vec2<f32>(-0.5, 0.5),   // Top-left
        vec2<f32>(-0.5, 0.5),   // Top-left
        vec2<f32>(0.5, -0.5),   // Bottom-right
        vec2<f32>(0.5, 0.5)     // Top-right
    );

    // Texture coordinates (0,0 is top-left, 1,1 is bottom-right)
    var tex_coords = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),  // Bottom-left
        vec2<f32>(1.0, 1.0),  // Bottom-right
        vec2<f32>(0.0, 0.0),  // Top-left
        vec2<f32>(0.0, 0.0),  // Top-left
        vec2<f32>(1.0, 1.0),  // Bottom-right
        vec2<f32>(1.0, 0.0)   // Top-right
    );

    // Quad UV for rounded corners (0 to 1 within the quad)
    var quad_uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),  // Bottom-left
        vec2<f32>(1.0, 0.0),  // Bottom-right
        vec2<f32>(0.0, 1.0),  // Top-left
        vec2<f32>(0.0, 1.0),  // Top-left
        vec2<f32>(1.0, 0.0),  // Bottom-right
        vec2<f32>(1.0, 1.0)   // Top-right
    );

    let unit_pos = unit_positions[in_vertex_index];

    // Scale the unit quad by the size
    var scaled_pos = unit_pos * camera_settings.size;

    // Offset to position (camera_settings.position is the center of the quad)
    let final_pos = scaled_pos + camera_settings.position;

    out.position = vec4<f32>(final_pos.x, final_pos.y, 0.0, 1.0);
    out.tex_coords = tex_coords[in_vertex_index];
    out.quad_uv = quad_uvs[in_vertex_index];

    return out;
}

// Signed distance function for a rounded rectangle
fn rounded_box_sdf(point: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(point) - size + vec2<f32>(radius);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the camera texture
    let camera_color = textureSample(my_texture, my_sampler, in.tex_coords);

    // Calculate rounded corner mask
    // Convert quad_uv from (0,1) to (-0.5, 0.5) centered coordinates
    var centered_uv = in.quad_uv - vec2<f32>(0.5);

    // Correct for aspect ratio to get circular corners
    centered_uv.x *= camera_settings.aspect_ratio;

    // Calculate the SDF for the rounded rectangle
    let half_size = vec2<f32>(0.5 * camera_settings.aspect_ratio, 0.5);
    let corner_radius = camera_settings.corner_radius;

    let dist = rounded_box_sdf(centered_uv, half_size, corner_radius);

    // Anti-aliased edge using fwidth for smooth edges
    let pixel_width = length(fwidth(in.quad_uv));
    let aa_width = max(pixel_width * 1.5, 0.005);

    // Create smooth alpha mask
    let alpha = 1.0 - smoothstep(-aa_width, aa_width, dist);

    // Discard fully transparent pixels
    if (alpha < 0.01) {
        discard;
    }

    return vec4<f32>(camera_color.rgb, camera_color.a * alpha);
}
