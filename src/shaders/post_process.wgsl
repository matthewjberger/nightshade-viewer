
@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    var pos: vec2<f32>;

    switch(vertex_index) {
        case 0u: {
            pos = vec2<f32>(-1.0, -1.0);
        }
        case 1u: {
            pos = vec2<f32>(3.0, -1.0);
        }
        default: {
            pos = vec2<f32>(-1.0, 3.0);
        }
    }

    output.position = vec4<f32>(pos, 0.0, 1.0);
    output.uv = pos * vec2<f32>(0.5, -0.5) + 0.5;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let r = textureSample(scene_texture, scene_sampler, input.uv).r;
    let g = textureSample(scene_texture, scene_sampler, input.uv).g;
    let b = textureSample(scene_texture, scene_sampler, input.uv).b;
    return vec4<f32>(r, g, b, 1.0);
}
