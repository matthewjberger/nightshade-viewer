use crate::ecs::ViewerWorld;
use nightshade::prelude::*;

/// Sets the selection, syncs it to the engine's outline pass, and flags a
/// selection update for the page.
pub fn select(viewer: &mut ViewerWorld, world: &mut World, entity: Option<Entity>) {
    viewer.resources.selection.selected = entity;
    world
        .resources
        .editor_selection
        .bounding_volume_selected_entity = entity;
    world.resources.editor_selection.selected_entities = entity.into_iter().collect();
    viewer.resources.scene_sync.needs_selection = true;
}

/// Selects the model entity with the given raw id (from the scene tree).
pub fn select_by_id(viewer: &mut ViewerWorld, world: &mut World, id: u32) {
    let entity = find_entity_by_id(world, id);
    select(viewer, world, entity);
}

/// Applies an edited transform (Euler degrees) to a model entity.
pub fn set_transform(
    world: &mut World,
    id: u32,
    translation: [f32; 3],
    rotation: [f32; 3],
    scale: [f32; 3],
) {
    let Some(entity) = find_entity_by_id(world, id) else {
        return;
    };
    if let Some(transform) = world.core.get_local_transform_mut(entity) {
        transform.translation = Vec3::new(translation[0], translation[1], translation[2]);
        transform.rotation = euler_to_quat(rotation);
        transform.scale = Vec3::new(scale[0], scale[1], scale[2]);
    }
    mark_local_transform_dirty(world, entity);
}

/// Finds a transformable entity by its raw id.
pub fn find_entity_by_id(world: &World, id: u32) -> Option<Entity> {
    world
        .core
        .query_entities(LOCAL_TRANSFORM)
        .find(|entity| entity.id == id)
}

/// Quaternion to XYZ Euler angles in degrees.
pub fn quat_to_euler(rotation: nalgebra_glm::Quat) -> [f32; 3] {
    let coords = rotation.coords;
    let (x, y, z, w) = (coords[0], coords[1], coords[2], coords[3]);

    let sinr_cosp = 2.0 * (w * x + y * z);
    let cosr_cosp = 1.0 - 2.0 * (x * x + y * y);
    let roll = sinr_cosp.atan2(cosr_cosp);

    let sinp = 2.0 * (w * y - z * x);
    let pitch = if sinp.abs() >= 1.0 {
        (std::f32::consts::FRAC_PI_2).copysign(sinp)
    } else {
        sinp.asin()
    };

    let siny_cosp = 2.0 * (w * z + x * y);
    let cosy_cosp = 1.0 - 2.0 * (y * y + z * z);
    let yaw = siny_cosp.atan2(cosy_cosp);

    [roll.to_degrees(), pitch.to_degrees(), yaw.to_degrees()]
}

/// XYZ Euler angles in degrees to a quaternion.
pub fn euler_to_quat(degrees: [f32; 3]) -> nalgebra_glm::Quat {
    let (cr, sr) = half_angle(degrees[0]);
    let (cp, sp) = half_angle(degrees[1]);
    let (cy, sy) = half_angle(degrees[2]);

    let w = cr * cp * cy + sr * sp * sy;
    let x = sr * cp * cy - cr * sp * sy;
    let y = cr * sp * cy + sr * cp * sy;
    let z = cr * cp * sy - sr * sp * cy;
    nalgebra_glm::Quat::new(w, x, y, z)
}

fn half_angle(degrees: f32) -> (f32, f32) {
    let half = degrees.to_radians() * 0.5;
    (half.cos(), half.sin())
}
