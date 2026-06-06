use crate::ecs::ViewerWorld;
use nightshade::prelude::*;

/// Runs the engine pan-orbit controller (which reads the forwarded pointer
/// input and respects the gizmo's `hud_wants_pointer`) and frames the model when
/// requested.
pub fn update(viewer: &mut ViewerWorld, world: &mut World) {
    if std::mem::take(&mut viewer.resources.camera_input.frame_requested) {
        frame_model(viewer, world);
    }
    pan_orbit_camera_system(world);
}

/// Frames the camera on the whole loaded model by fitting its bounds.
pub fn frame_model(viewer: &mut ViewerWorld, world: &mut World) {
    let Some(camera) = world.resources.active_camera else {
        return;
    };

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

    if !found {
        return;
    }

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
    let radius = half_diagonal / (fov * 0.5).sin() * 1.4;

    if let Some(orbit) = world.core.get_pan_orbit_camera_mut(camera) {
        orbit.target_focus = center;
        orbit.target_radius = radius;
    }
}
