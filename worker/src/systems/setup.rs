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

    spawn_sun(world);

    let camera = spawn_pan_orbit_camera(
        world,
        Vec3::new(0.0, 0.0, 0.0),
        4.0,
        0.7,
        0.5,
        "Camera".to_string(),
    );
    world.resources.active_camera = Some(camera);
    world.core.add_components(camera, VIEWPORT_SHADING);
    world.core.set_viewport_shading(
        camera,
        nightshade::ecs::camera::components::ViewportShading::default(),
    );

    load::load_default(viewer, world);
}
