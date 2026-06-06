use crate::ecs::ViewerWorld;
use nightshade::prelude::*;
use protocol::{AssetKind, WorkerMessage};

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
    match asset.kind {
        AssetKind::Model => load_model(viewer, world, &asset.bytes),
        AssetKind::Hdri => load_hdri(world, &asset.bytes),
    }
    crate::post(&WorkerMessage::Loading {
        active: false,
        label: String::new(),
    });
}

/// Imports a glTF/GLB, replaces the current model, records its entities, and
/// frames the camera on it.
pub fn load_model(viewer: &mut ViewerWorld, world: &mut World, bytes: &[u8]) {
    despawn_current(viewer, world);

    let mut result = match import_gltf_from_bytes(bytes) {
        Ok(result) => result,
        Err(error) => {
            tracing::error!("failed to import model: {error}");
            return;
        }
    };
    nightshade::ecs::loading::queue_gltf_load(world, &mut result);
    let Some(prefab) = result.prefabs.first() else {
        return;
    };

    let root = nightshade::ecs::prefab::spawn_prefab(world, prefab, Vec3::new(0.0, 0.0, 0.0));
    let mut entities = nightshade::ecs::transform::queries::query_descendants(world, root);
    entities.insert(0, root);

    viewer.resources.model.root = Some(root);
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
    if let Some(root) = viewer.resources.model.root.take() {
        despawn_recursive_immediate(world, root);
    }
    viewer.resources.model.entities.clear();
    world.resources.mesh_render_state.request_full_rebuild();
}
