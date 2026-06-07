use crate::ecs::ViewerWorld;
use crate::systems::selection;
use nightshade::ecs::material::components::Material;
use nightshade::prelude::*;
use protocol::{LightKind, PrimitiveKind};

/// Spawns a primitive mesh at the camera focus, selects it, and returns it.
pub fn add_primitive(viewer: &mut ViewerWorld, world: &mut World, kind: PrimitiveKind) -> Entity {
    let position = focus(world);
    let entity = match kind {
        PrimitiveKind::Cube => spawn_cube_at(world, position),
        PrimitiveKind::Sphere => spawn_sphere_at(world, position),
        PrimitiveKind::Cylinder => spawn_cylinder_at(world, position),
        PrimitiveKind::Cone => spawn_cone_at(world, position),
        PrimitiveKind::Torus => spawn_torus_at(world, position),
        PrimitiveKind::Plane => spawn_plane_at(world, position),
    };
    register(viewer, world, entity);
    entity
}

/// Spawns a light above the camera focus, selects it, and returns it. With
/// `marker`, a small emissive sphere makes it visible and pickable in the
/// viewport (the toolbar button). Without it (the agent path) the light is a
/// bare entity, so nothing renders a stray emissive sphere that blooms huge once
/// the agent rescales it with its own transform.
pub fn add_light(
    viewer: &mut ViewerWorld,
    world: &mut World,
    kind: LightKind,
    marker: bool,
) -> Entity {
    let position = focus(world) + Vec3::new(0.0, 2.0, 0.0);
    let color = Vec3::new(1.0, 0.95, 0.8);
    let (name, light_type, intensity, range) = match kind {
        LightKind::Directional => ("Directional Light", LightType::Directional, 3.0, 0.0),
        LightKind::Point => ("Point Light", LightType::Point, 5.0, 10.0),
        LightKind::Spot => ("Spot Light", LightType::Spot, 10.0, 15.0),
    };

    let entity = if marker {
        let entity = spawn_sphere_at(world, position);
        if let Some(transform) = world.core.get_local_transform_mut(entity) {
            transform.scale = Vec3::new(0.15, 0.15, 0.15);
        }
        mark_local_transform_dirty(world, entity);
        spawn_material(
            world,
            entity,
            format!("LightMarker_{}", entity.id),
            Material {
                base_color: [color.x, color.y, color.z, 1.0],
                emissive_factor: [color.x, color.y, color.z],
                emissive_strength: 4.0,
                roughness: 1.0,
                metallic: 0.0,
                unlit: false,
                ..Default::default()
            },
        );
        entity
    } else {
        spawn_light_entity(world, position)
    };
    world.core.set_name(entity, Name(name.to_string()));
    world.core.add_components(entity, LIGHT);
    world.core.set_light(
        entity,
        Light {
            light_type,
            color,
            intensity,
            range,
            ..Default::default()
        },
    );
    register(viewer, world, entity);
    entity
}

/// Spawns a bare transformable entity (no mesh) at `position`, for a light that
/// needs no visible marker.
fn spawn_light_entity(world: &mut World, position: Vec3) -> Entity {
    let mask = LOCAL_TRANSFORM | GLOBAL_TRANSFORM | LOCAL_TRANSFORM_DIRTY;
    let entity = nightshade::ecs::world::commands::spawn_entities(world, mask, 1)
        .into_iter()
        .next()
        .expect("spawn light entity");
    if let Some(transform) = world.core.get_local_transform_mut(entity) {
        transform.translation = position;
    }
    mark_local_transform_dirty(world, entity);
    entity
}

/// The world-space point the active pan-orbit camera orbits, or the origin.
fn focus(world: &World) -> Vec3 {
    world
        .resources
        .active_camera
        .and_then(|camera| world.core.get_pan_orbit_camera(camera))
        .map(|orbit| orbit.target_focus)
        .unwrap_or_else(|| Vec3::new(0.0, 0.0, 0.0))
}

/// Adds a freshly spawned entity to the scene tree and selects it.
fn register(viewer: &mut ViewerWorld, world: &mut World, entity: Entity) {
    viewer.resources.model.entities.push(entity);
    viewer.resources.scene_sync.needs_tree = true;
    selection::select(viewer, world, Some(entity));
}
