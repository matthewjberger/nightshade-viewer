use std::collections::HashMap;

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

/// Replaces the current model with a freshly imported one. Spawns every prefab
/// with its skins and animations so skeleton joints are parented under the root
/// (and therefore despawn with it), then frames the camera.
fn spawn_result(viewer: &mut ViewerWorld, world: &mut World, mut result: GltfLoadResult) {
    despawn_current(viewer, world);
    nightshade::ecs::loading::queue_gltf_load(world, &mut result);

    let mut roots = Vec::new();
    let mut entities = Vec::new();
    for prefab in &result.prefabs {
        let root = nightshade::ecs::prefab::spawn_prefab_with_skins(
            world,
            prefab,
            &result.animations,
            &result.skins,
            Vec3::new(0.0, 0.0, 0.0),
        );
        entities.push(root);
        entities.extend(nightshade::ecs::transform::queries::query_descendants(
            world, root,
        ));
        roots.push(root);
    }
    if roots.is_empty() {
        return;
    }

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

fn load_hdri(world: &mut World, bytes: &[u8]) {
    world.resources.render_settings.atmosphere = Atmosphere::Hdr;
    load_hdr_skybox(world, bytes.to_vec());
}

fn despawn_current(viewer: &mut ViewerWorld, world: &mut World) {
    for root in std::mem::take(&mut viewer.resources.model.roots) {
        despawn_recursive_immediate(world, root);
    }
    viewer.resources.model.entities.clear();
    world.resources.mesh_render_state.request_full_rebuild();
}
