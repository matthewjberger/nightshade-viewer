use crate::ecs::ViewerWorld;
use nightshade::prelude::*;

/// Angular speed of the turntable: one full revolution every twenty seconds.
const TURNTABLE_RADIANS_PER_SECOND: f32 = std::f32::consts::TAU / 20.0;

/// Runs the engine pan-orbit controller (which reads the forwarded pointer
/// input and respects the gizmo's `hud_wants_pointer`), frames the model when
/// requested, and advances the turntable when it is enabled.
pub fn update(viewer: &mut ViewerWorld, world: &mut World) {
    if std::mem::take(&mut viewer.resources.camera_input.frame_requested) {
        frame_model(viewer, world);
    }
    if viewer.resources.camera_input.turntable {
        advance_turntable(world);
    }
    pan_orbit_camera_system(world);
}

/// Eases the orbit yaw target forward each frame so the controller's own
/// smoothing carries the camera around the model at a constant rate.
fn advance_turntable(world: &mut World) {
    let Some(camera) = world.resources.active_camera else {
        return;
    };
    let delta_time = world.resources.window.timing.delta_time;
    if let Some(orbit) = world.core.get_pan_orbit_camera_mut(camera) {
        orbit.target_yaw += TURNTABLE_RADIANS_PER_SECOND * delta_time;
    }
}

/// The loaded model's world-space bounding box, from each entity's bounding
/// volume and global transform.
pub fn model_bounds(viewer: &ViewerWorld, world: &World) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut found = false;

    for &entity in &viewer.resources.model.entities {
        let Some(global) = world.core.get_global_transform(entity) else {
            continue;
        };
        let position = global.0.column(3).xyz();
        let world_scale = global.0.column(0).xyz().norm();
        let radius = world
            .core
            .get_bounding_volume(entity)
            .map(|volume| volume.sphere_radius * world_scale)
            .unwrap_or(0.0);
        let offset = Vec3::new(radius, radius, radius);
        min = nalgebra_glm::min2(&min, &(position - offset));
        max = nalgebra_glm::max2(&max, &(position + offset));
        found = true;
    }

    found.then_some((min, max))
}

/// Frames the camera on the whole loaded model by fitting its bounds.
pub fn frame_model(viewer: &mut ViewerWorld, world: &mut World) {
    let Some(camera) = world.resources.active_camera else {
        return;
    };
    let Some((min, max)) = model_bounds(viewer, world) else {
        return;
    };

    let center = (min + max) * 0.5;
    let half_diagonal = ((max - min) * 0.5).norm().max(0.01);
    let fov = world
        .core
        .get_camera(camera)
        .and_then(|camera| match camera.projection {
            Projection::Perspective(perspective) => Some(perspective.y_fov_rad),
            _ => None,
        })
        .unwrap_or(45.0_f32.to_radians());
    let radius = half_diagonal / (fov * 0.5).sin() * 1.2;

    let near = (half_diagonal * 0.001).clamp(0.001, 0.1);
    if let Some(camera_component) = world.core.get_camera_mut(camera)
        && let Projection::Perspective(perspective) = &mut camera_component.projection
    {
        perspective.z_near = near;
    }

    if let Some(orbit) = world.core.get_pan_orbit_camera_mut(camera) {
        orbit.target_focus = center;
        orbit.target_radius = radius;
    }
}
