use std::collections::HashSet;

use crate::ecs::ViewerWorld;
use crate::systems::selection;
use nightshade::prelude::*;

/// Requests a GPU pick at a pixel, unless a gizmo handle is being dragged.
pub fn request(viewer: &mut ViewerWorld, world: &mut World, x: f32, y: f32) {
    if gizmo_drag_active(world) {
        return;
    }
    world.resources.gpu_picking = GpuPicking::default();
    world
        .resources
        .gpu_picking
        .request_pick(x.max(0.0) as u32, y.max(0.0) as u32);
    viewer.resources.picking.pending = true;
}

/// Polls the pending pick. Hitting a model entity selects it, and clicking the
/// same spot again cycles from the model root down toward the leaf. Background
/// clears the selection.
pub fn apply(viewer: &mut ViewerWorld, world: &mut World) {
    if !viewer.resources.picking.pending {
        return;
    }
    let Some(result) = world.resources.gpu_picking.take_result() else {
        return;
    };
    viewer.resources.picking.pending = false;
    if gizmo_drag_active(world) {
        return;
    }

    let leaf = if result.depth > 0.0 {
        result
            .entity_id
            .and_then(|id| selection::find_entity_by_id(world, id))
    } else {
        None
    };

    match leaf {
        Some(leaf) => cycle_select(viewer, world, leaf),
        None => {
            reset_cycle(viewer);
            selection::select(viewer, world, None);
        }
    }
}

fn cycle_select(viewer: &mut ViewerWorld, world: &mut World, leaf: Entity) {
    let ids: HashSet<u32> = viewer
        .resources
        .model
        .entities
        .iter()
        .map(|entity| entity.id)
        .collect();
    let root = find_root(world, leaf, &ids);
    let chain = chain_from_root(world, root, leaf);
    if chain.is_empty() {
        return;
    }

    let picking = &viewer.resources.picking;
    let same_target = picking.last_leaf == Some(leaf) && picking.last_root == Some(root);
    let selection_on_chain = viewer
        .resources
        .selection
        .selected
        .is_some_and(|selected| chain.contains(&selected));
    let depth = if same_target && selection_on_chain {
        (picking.cycle_depth + 1) % chain.len()
    } else {
        0
    };

    viewer.resources.picking.last_leaf = Some(leaf);
    viewer.resources.picking.last_root = Some(root);
    viewer.resources.picking.cycle_depth = depth;
    selection::select(viewer, world, Some(chain[depth]));
}

fn reset_cycle(viewer: &mut ViewerWorld) {
    viewer.resources.picking.last_leaf = None;
    viewer.resources.picking.last_root = None;
    viewer.resources.picking.cycle_depth = 0;
}

/// Walks up parents while they remain inside the model to find the group root.
fn find_root(world: &World, leaf: Entity, ids: &HashSet<u32>) -> Entity {
    let mut current = leaf;
    let mut steps = 0;
    while steps < 64 {
        match world.core.get_parent(current) {
            Some(Parent(Some(parent))) if ids.contains(&parent.id) => {
                current = *parent;
                steps += 1;
            }
            _ => break,
        }
    }
    current
}

/// Builds the chain from the root down to the leaf (root first).
fn chain_from_root(world: &World, root: Entity, leaf: Entity) -> Vec<Entity> {
    let mut chain = vec![leaf];
    let mut current = leaf;
    let mut steps = 0;
    while current != root && steps < 64 {
        match world.core.get_parent(current) {
            Some(Parent(Some(parent))) => {
                chain.push(*parent);
                current = *parent;
                steps += 1;
            }
            _ => break,
        }
    }
    chain.reverse();
    chain
}

fn gizmo_drag_active(world: &World) -> bool {
    let gizmos = &world.resources.user_interface.gizmos;
    gizmos.translation_drag.is_some()
        || gizmos.scale_drag.is_some()
        || gizmos.rotation_drag.is_some()
        || gizmos.planar_scale_drag.is_some()
        || gizmos.planar_translation_drag.is_some()
        || gizmos.nav_gizmo_drag.is_some()
}
