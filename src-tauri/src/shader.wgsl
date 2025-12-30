// Camera settings uniform buffer
struct CameraSettings {
    // Position of the camera quad center in NDC space (-1 to 1)
    position: vec2<f32>,
    // Size of the camera quad in NDC space (0 to 2)
    size: vec2<f32>,

};

@group(0) @binding(0) var my_texture: texture_2d<f32>;
@group(0) @binding(1) var my_sampler: sampler;
@group(1) @binding(0) var<uniform> camera_settings: CameraSettings;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Two triangles forming a quad
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

    let unit_pos = unit_positions[in_vertex_index];
    let scaled_pos = unit_pos * camera_settings.size;
    let final_pos = scaled_pos + camera_settings.position;

    out.position = vec4<f32>(final_pos.x, final_pos.y, 0.0, 1.0);
    out.tex_coords = tex_coords[in_vertex_index];

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(my_texture, my_sampler, in.tex_coords);
}
