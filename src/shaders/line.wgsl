struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) start: vec4<f32>,
    @location(2) end: vec4<f32>,
    @location(3) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

struct Uniforms {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Interpolate between start and end based on vertex position
    let pos = mix(model.start.xyz, model.end.xyz, model.position.x);

    out.clip_position = uniforms.view_proj * vec4<f32>(pos, 1.0);
    out.color = model.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
