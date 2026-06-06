use crate::ecs::ViewerWorld;
use nightshade::prelude::*;

const ORBIT_SENSITIVITY: f32 = 0.005;
const ZOOM_SENSITIVITY: f32 = 0.12;
const PAN_SENSITIVITY: f32 = 0.0015;
const PITCH_LIMIT: f32 = 1.55;

/// Applies forwarded orbit, pan, and zoom deltas to the pan-orbit camera, runs
/// the engine controller, and frames the model when requested.
pub fn update(viewer: &mut ViewerWorld, world: &mut World) {
    let input = &mut viewer.resources.camera_input;
    let yaw = input.pending_yaw;
    let pitch = input.pending_pitch;
    let pan_x = input.pending_pan_x;
    let pan_y = input.pending_pan_y;
    let zoom = input.pending_zoom;
    let frame = std::mem::take(&mut input.frame_requested);
    input.pending_yaw = 0.0;
    input.pending_pitch = 0.0;
    input.pending_pan_x = 0.0;
    input.pending_pan_y = 0.0;
    input.pending_zoom = 0.0;

    if frame {
        frame_model(viewer, world);
    }

    let Some(camera) = world.resources.active_camera else {
        pan_orbit_camera_system(world);
        return;
    };

    let basis = if pan_x != 0.0 || pan_y != 0.0 {
        world.core.get_global_transform(camera).map(|global| {
            (
                global.0.column(0).xyz().normalize(),
                global.0.column(1).xyz().normalize(),
            )
        })
    } else {
        None
    };

    if let Some(orbit) = world.core.get_pan_orbit_camera_mut(camera) {
        orbit.target_yaw -= yaw * ORBIT_SENSITIVITY;
        orbit.target_pitch =
            (orbit.target_pitch + pitch * ORBIT_SENSITIVITY).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        let factor = (1.0 + ZOOM_SENSITIVITY).powf(zoom);
        orbit.target_radius = (orbit.target_radius * factor).clamp(0.05, 5000.0);
        if let Some((right, up)) = basis {
            let scale = orbit.target_radius * PAN_SENSITIVITY;
            orbit.target_focus += right * (-pan_x * scale) + up * (pan_y * scale);
        }
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
