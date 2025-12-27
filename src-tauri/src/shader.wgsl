@group(0) @binding(0) var my_texture: texture_2d<f32>;
@group(0) @binding(1) var my_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Full-screen quad vertices (two triangles)
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),  // Bottom-left
        vec2<f32>(1.0, -1.0),   // Bottom-right
        vec2<f32>(-1.0, 1.0),   // Top-left
        vec2<f32>(-1.0, 1.0),   // Top-left
        vec2<f32>(1.0, -1.0),   // Bottom-right
        vec2<f32>(1.0, 1.0)     // Top-right
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

    let pos = positions[in_vertex_index];
    out.position = vec4<f32>(pos.x, pos.y, 0.0, 1.0);
    out.tex_coords = tex_coords[in_vertex_index];

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(my_texture, my_sampler, in.tex_coords);
}
