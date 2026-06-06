use crate::ecs::ViewerWorld;
use crate::systems::selection;
use nightshade::prelude::*;

/// Requests a GPU pick at a pixel. The picking resource is reset first so the
/// engine returns an unsmoothed, single-click result.
pub fn request(viewer: &mut ViewerWorld, world: &mut World, x: f32, y: f32) {
    world.resources.gpu_picking = GpuPicking::default();
    world
        .resources
        .gpu_picking
        .request_pick(x.max(0.0) as u32, y.max(0.0) as u32);
    viewer.resources.picking.pending = true;
}

/// Polls the pending pick. Selects the hit entity, or clears the selection when
/// the background was clicked.
pub fn apply(viewer: &mut ViewerWorld, world: &mut World) {
    if !viewer.resources.picking.pending {
        return;
    }
    let Some(result) = world.resources.gpu_picking.take_result() else {
        return;
    };
    viewer.resources.picking.pending = false;

    let picked = if result.depth > 0.0 {
        result
            .entity_id
            .and_then(|id| selection::find_entity_by_id(world, id))
    } else {
        None
    };
    selection::select(viewer, world, picked);
}
