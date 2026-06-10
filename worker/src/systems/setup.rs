use crate::ecs::ViewerWorld;
use crate::systems::load;
use nightshade::prelude::*;

/// Builds the scene: environment, lighting, camera, render settings, selection
/// outline, and the default model.
pub fn spawn(viewer: &mut ViewerWorld, world: &mut World) {
    world.resources.user_interface.enabled = true;
    world.resources.retained_ui.enabled = true;
    world.resources.user_interface.gizmos.nav_gizmo_enabled = false;
    if let Some((width, height)) = world.resources.window.cached_viewport_size {
        world.resources.window.active_viewport_rect =
            Some(nightshade::ecs::window::resources::ViewportRect {
                x: 0.0,
                y: 0.0,
                width: width as f32,
                height: height as f32,
            });
    }
    world.resources.render_settings.atmosphere = Atmosphere::Sky;
    world.resources.render_settings.clear_color = [0.09, 0.10, 0.13, 1.0];
    capture_procedural_atmosphere_ibl(world, Atmosphere::Sky, 0.0);
    world.resources.render_settings.ssao_enabled = true;
    world.resources.render_settings.bloom_enabled = true;
    world.resources.debug_draw.show_grid = true;
    world.resources.debug_draw.selection_outline_enabled = true;
    world.resources.debug_draw.selection_outline_color = [1.0, 0.5, 0.15, 1.0];

    nightshade::ecs::world::commands::load_procedural_textures(world);
    nightshade::ecs::loading::load_texture_pack_from_image_bytes(
        world,
        &[
            (
                "greybox_light",
                include_bytes!("../../assets/textures/greybox_light.png"),
            ),
            (
                "greybox_dark",
                include_bytes!("../../assets/textures/greybox_dark.png"),
            ),
        ],
        nightshade::render::wgpu::texture_cache::TextureUsage::Color,
        nightshade::render::wgpu::texture_cache::SamplerSettings::DEFAULT,
    );
    // The engine evicts zero-reference texture-cache entries on every load
    // drain and only glTF imports protect their names. Materials reference
    // these by name without reference counting, so protect them the same way
    // or any load (an HDRI, a model) evicts them for good.
    for name in [
        "checkerboard",
        "gradient",
        "uv_test",
        "greybox_light",
        "greybox_dark",
    ] {
        nightshade::render::wgpu::texture_cache::texture_cache_protect(
            &mut world.resources.texture_cache,
            name.to_string(),
        );
    }

    let sun = spawn_sun(world);
    if let Some(light) = world.core.get_light_mut(sun) {
        light.cast_shadows = true;
        light.intensity = 3.5;
        light.shadow_bias = 0.008;
    }

    crate::systems::camera::ensure_active(world);

    load::load_default(viewer, world);
}
