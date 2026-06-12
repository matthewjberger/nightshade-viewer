use std::collections::HashSet;

use crate::ecs::ViewerWorld;
use nightshade::ecs::lines::components::Line;
use nightshade::prelude::*;

const BONE_COLOR: Vec4 = Vec4::new(1.0, 0.65, 0.15, 1.0);

/// Draws every skin's joint hierarchy as lines while the overlay is enabled.
/// The lines entity persists across model loads and is rebuilt each frame from
/// the joints' global transforms, so it follows playing animations.
pub fn update(viewer: &mut ViewerWorld, world: &mut World) {
    if !viewer.resources.skeleton.enabled {
        if let Some(entity) = viewer.resources.skeleton.entity
            && let Some(visibility) = world.core.get_visibility_mut(entity)
            && visibility.visible
        {
            visibility.visible = false;
            if let Some(lines) = world.core.get_lines_mut(entity) {
                lines.clear();
            }
        }
        return;
    }

    let entity = match viewer.resources.skeleton.entity {
        Some(entity) if world.core.entity_has_components(entity, LINES) => entity,
        _ => {
            let entity = spawn_entities(
                world,
                LINES
                    | VISIBILITY
                    | LOCAL_TRANSFORM
                    | GLOBAL_TRANSFORM
                    | LOCAL_TRANSFORM_DIRTY
                    | NAME,
                1,
            )[0];
            world
                .core
                .set_name(entity, Name("Skeleton Overlay".to_string()));
            if let Some(lines) = world.core.get_lines_mut(entity) {
                lines.always_on_top = true;
            }
            viewer.resources.skeleton.entity = Some(entity);
            entity
        }
    };

    if let Some(visibility) = world.core.get_visibility_mut(entity) {
        visibility.visible = true;
    }

    let segments = bone_segments(world);
    if let Some(lines) = world.core.get_lines_mut(entity) {
        lines.clear();
        for (start, end) in segments {
            lines.push(Line {
                start,
                end,
                color: BONE_COLOR,
            });
        }
    }
}

/// Collects one world-space segment per bone: from each joint's parent joint to
/// the joint itself, deduplicated across skins that share joints.
fn bone_segments(world: &World) -> Vec<(Vec3, Vec3)> {
    let mut seen: HashSet<(u32, u32)> = HashSet::new();
    let mut segments = Vec::new();
    for skin_entity in world.core.query_entities(SKIN) {
        let Some(skin) = world.core.get_skin(skin_entity) else {
            continue;
        };
        for &joint in skin.joints.iter() {
            let Some(Parent(Some(parent))) = world.core.get_parent(joint) else {
                continue;
            };
            let parent = *parent;
            if !world.core.entity_has_components(parent, JOINT) {
                continue;
            }
            if !seen.insert((parent.id, joint.id)) {
                continue;
            }
            let (Some(parent_transform), Some(joint_transform)) = (
                world.core.get_global_transform(parent),
                world.core.get_global_transform(joint),
            ) else {
                continue;
            };
            segments.push((
                parent_transform.translation(),
                joint_transform.translation(),
            ));
        }
    }
    segments
}
