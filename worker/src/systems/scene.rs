use crate::ecs::ViewerWorld;
use crate::systems::selection::quat_to_euler;
use nightshade::prelude::*;
use protocol::{EntityDetail, SceneNode, WorkerMessage};

/// Posts the scene tree and the selection detail to the page when flagged.
pub fn sync(viewer: &mut ViewerWorld, world: &mut World) {
    if std::mem::take(&mut viewer.resources.scene_sync.needs_tree) {
        let nodes = build_tree(viewer, world);
        crate::post(&WorkerMessage::Scene { nodes });
    }
    if std::mem::take(&mut viewer.resources.scene_sync.needs_selection) {
        let detail = viewer
            .resources
            .selection
            .selected
            .and_then(|entity| build_detail(world, entity));
        crate::post(&WorkerMessage::Selected { detail });
    }
}

fn build_tree(viewer: &ViewerWorld, world: &World) -> Vec<SceneNode> {
    let root = viewer.resources.model.root;
    viewer
        .resources
        .model
        .entities
        .iter()
        .map(|&entity| {
            let name = world
                .core
                .get_name(entity)
                .map(|name| name.0.clone())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| format!("Node {}", entity.id));
            SceneNode {
                id: entity.id,
                name,
                depth: depth_of(world, entity, root),
                has_mesh: world.core.entity_has_components(entity, RENDER_MESH),
            }
        })
        .collect()
}

fn depth_of(world: &World, entity: Entity, root: Option<Entity>) -> u32 {
    let mut depth = 0;
    let mut current = entity;
    while Some(current) != root && depth < 64 {
        match world.core.get_parent(current) {
            Some(Parent(Some(parent))) => {
                current = *parent;
                depth += 1;
            }
            _ => break,
        }
    }
    depth
}

/// Builds the inspector detail for an entity.
pub fn build_detail(world: &World, entity: Entity) -> Option<EntityDetail> {
    let transform = world.core.get_local_transform(entity)?;
    let translation = transform.translation;
    let scale = transform.scale;
    let name = world
        .core
        .get_name(entity)
        .map(|name| name.0.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("Node {}", entity.id));
    let mesh = world
        .core
        .get_render_mesh(entity)
        .map(|mesh| mesh.name.clone());
    Some(EntityDetail {
        id: entity.id,
        name,
        translation: [translation.x, translation.y, translation.z],
        rotation: quat_to_euler(transform.rotation),
        scale: [scale.x, scale.y, scale.z],
        mesh,
    })
}
