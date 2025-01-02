@group(0) @binding(0)
var equirect_texture: texture_2d<f32>;

@group(0) @binding(1)
var equirect_sampler: sampler;

@group(0) @binding(2)
var output_texture: texture_storage_2d_array<rgba32float, write>;

const PI: f32 = 3.141592653589793;
const FACE_SIZE: u32 = 1024u;

// Convert normalized cube coordinates to world direction vector
fn cube_to_world(face: u32, uv: vec2<f32>) -> vec3<f32> {
    var dir: vec3<f32>;
    let x = 2.0 * uv.x - 1.0;
    let y = 2.0 * uv.y - 1.0;

    switch face {
        case 0u: { // +X (right)
            dir = vec3<f32>(1.0, -y, -x);
        }
        case 1u: { // -X (left)
            dir = vec3<f32>(-1.0, -y, x);
        }
        case 2u: { // +Y (top)
            dir = vec3<f32>(x, 1.0, y);
        }
        case 3u: { // -Y (bottom)
            dir = vec3<f32>(x, -1.0, -y);
        }
        case 4u: { // +Z (front)
            dir = vec3<f32>(x, -y, 1.0);
        }
        default: { // -Z (back)
            dir = vec3<f32>(-x, -y, -1.0);
        }
    }
    return normalize(dir);
}

// Convert world direction to equirectangular UV coordinates
fn world_to_equirect(dir: vec3<f32>) -> vec2<f32> {
    let phi = atan2(dir.z, dir.x);
    let theta = asin(dir.y);
    
    var uv = vec2<f32>(phi / (2.0 * PI), theta / PI);
    uv.x = uv.x + 0.5;
    uv.y = 0.5 - uv.y;
    return uv;
}

@compute @workgroup_size(16, 16, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>
) {
    let face = group_id.z;
    if face >= 6u {
        return;
    }

    let coords = vec2<u32>(global_id.xy);
    if coords.x >= FACE_SIZE || coords.y >= FACE_SIZE {
        return;
    }

    // Convert pixel coordinates to normalized coordinates
    let uv = vec2<f32>(coords) / f32(FACE_SIZE);
    
    // Get world direction for this pixel
    let dir = cube_to_world(face, uv);
    
    // Convert to equirectangular coordinates
    let equirect_uv = world_to_equirect(dir);
    
    // Sample the equirectangular texture
    let color = textureSampleLevel(
        equirect_texture,
        equirect_sampler,
        equirect_uv,
        0.0
    );
    
    // Write to the output cubemap face
    textureStore(
        output_texture,
        coords,
        face,
        color
    );
}