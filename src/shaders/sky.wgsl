struct Uniform {
    proj: mat4x4<f32>,
    proj_inv: mat4x4<f32>,
    view: mat4x4<f32>,
    cam_pos: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> u: Uniform;

@group(0) @binding(1)
var t_diffuse: texture_cube<f32>;

@group(0) @binding(2)
var s_diffuse: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec3<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let tmp1 = i32(vertex_index) / 2;
    let tmp2 = i32(vertex_index) & 1;
    let pos = vec4<f32>(
        f32(tmp1) * 4.0 - 1.0,
        f32(tmp2) * 4.0 - 1.0,
        1.0,
        1.0
    );

    // transposition = inversion for this orthonormal matrix
    let inv_model_view = transpose(mat3x3<f32>(u.view[0].xyz, u.view[1].xyz, u.view[2].xyz));
    let unprojected = u.proj_inv * pos;

    var result: VertexOutput;
    result.uv = inv_model_view * unprojected.xyz;
    result.position = pos;
    return result;
}

fn tone_map(hdr: vec3<f32>) -> vec3<f32> {
    let exposure = 1.0;
    let gamma = 2.2;
    
    // Apply exposure
    var color = vec3<f32>(1.0) - exp(-hdr * exposure);
    
    // Apply gamma correction
    color = pow(color, vec3<f32>(1.0 / gamma));
    
    return color;
}

@fragment
fn fs_sky(in: VertexOutput) -> @location(0) vec4<f32> {
    let hdr = textureSample(t_diffuse, s_diffuse, normalize(in.uv)).rgb;
    let color = tone_map(hdr);
    return vec4<f32>(color, 1.0);
}
