use std::collections::{HashMap, HashSet};

use crate::ecs::{PendingAsset, ViewerWorld};
use nightshade::ecs::prefab::GltfLoadResult;
use nightshade::prelude::*;
use protocol::WorkerMessage;

const DEFAULT_MODEL: &[u8] = include_bytes!("../../assets/DamagedHelmet.glb");

/// Loads the bundled default model on startup.
pub fn load_default(viewer: &mut ViewerWorld, world: &mut World) {
    load_model(viewer, world, DEFAULT_MODEL);
}

/// Applies a queued asset (from a drop or a browser fetch) once per frame.
pub fn poll(viewer: &mut ViewerWorld, world: &mut World) {
    let pending = viewer
        .resources
        .incoming
        .asset
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    let Some(asset) = pending else {
        return;
    };
    match asset {
        PendingAsset::Model(bytes) => load_model(viewer, world, &bytes),
        PendingAsset::ModelWithResources { gltf, resources } => {
            load_model_with_resources(viewer, world, &gltf, &resources)
        }
        PendingAsset::Hdri(bytes) => load_hdri(world, &bytes),
    }
    crate::post(&WorkerMessage::Loading {
        active: false,
        label: String::new(),
    });
}

/// Imports a self-contained glTF/GLB and spawns it.
pub fn load_model(viewer: &mut ViewerWorld, world: &mut World, bytes: &[u8]) {
    match import_gltf_from_bytes(bytes) {
        Ok(result) => spawn_result(viewer, world, result),
        Err(error) => tracing::error!("failed to import model: {error}"),
    }
}

/// Imports a glTF with external buffers and images supplied as bytes.
pub fn load_model_with_resources(
    viewer: &mut ViewerWorld,
    world: &mut World,
    gltf: &[u8],
    resources: &HashMap<String, Vec<u8>>,
) {
    match nightshade::ecs::prefab::import_gltf_with_resources(gltf, resources) {
        Ok(result) => spawn_result(viewer, world, result),
        Err(error) => tracing::error!("failed to import model: {error}"),
    }
}

/// Replaces the current model with a freshly imported one. Captures the exact
/// set of entities the spawn produced (by diffing the transformable entities
/// before and after) so the tree and the next despawn cover every spawned
/// entity, skin joints included, regardless of the engine's child cache state.
fn spawn_result(viewer: &mut ViewerWorld, world: &mut World, mut result: GltfLoadResult) {
    despawn_current(viewer, world);
    nightshade::ecs::loading::queue_gltf_load(world, &mut result);

    let before: HashSet<u32> = world
        .core
        .query_entities(LOCAL_TRANSFORM)
        .map(|entity| entity.id)
        .collect();

    let mut roots = Vec::new();
    for prefab in &result.prefabs {
        roots.push(nightshade::ecs::prefab::spawn_prefab_with_skins(
            world,
            prefab,
            &result.animations,
            &result.skins,
            Vec3::new(0.0, 0.0, 0.0),
        ));
    }
    if roots.is_empty() {
        return;
    }

    let spawned: HashSet<u32> = world
        .core
        .query_entities(LOCAL_TRANSFORM)
        .map(|entity| entity.id)
        .filter(|id| !before.contains(id))
        .collect();
    let entities = ordered_entities(world, &roots, &spawned);

    viewer.resources.model.roots = roots;
    viewer.resources.model.entities = entities;
    viewer.resources.selection.selected = None;
    world
        .resources
        .editor_selection
        .bounding_volume_selected_entity = None;
    world.resources.editor_selection.selected_entities.clear();
    viewer.resources.scene_sync.needs_tree = true;
    viewer.resources.scene_sync.needs_selection = true;
    viewer.resources.camera_input.frame_requested = true;
}

/// Orders the spawned set parent-before-child (a hierarchy walk from each root),
/// appending any entity not reached from a root at the end.
fn ordered_entities(world: &World, roots: &[Entity], spawned: &HashSet<u32>) -> Vec<Entity> {
    let mut ordered = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();
    for &root in roots {
        if spawned.contains(&root.id) && seen.insert(root.id) {
            ordered.push(root);
        }
        for descendant in nightshade::ecs::transform::queries::query_descendants(world, root) {
            if spawned.contains(&descendant.id) && seen.insert(descendant.id) {
                ordered.push(descendant);
            }
        }
    }
    for entity in world.core.query_entities(LOCAL_TRANSFORM) {
        if spawned.contains(&entity.id) && seen.insert(entity.id) {
            ordered.push(entity);
        }
    }
    ordered
}

fn load_hdri(world: &mut World, bytes: &[u8]) {
    world.resources.render_settings.atmosphere = Atmosphere::Hdr;
    load_hdr_skybox(world, bytes.to_vec());
}

/// Despawns every entity of the current model. Each tracked entity is despawned
/// individually (with a liveness check) so nothing survives even if a recursive
/// despawn from the root would have missed it.
fn despawn_current(viewer: &mut ViewerWorld, world: &mut World) {
    for entity in std::mem::take(&mut viewer.resources.model.entities) {
        if world.core.entity_has_components(entity, LOCAL_TRANSFORM) {
            despawn_recursive_immediate(world, entity);
        }
    }
    viewer.resources.model.roots.clear();
    world.resources.mesh_render_state.request_full_rebuild();
}
