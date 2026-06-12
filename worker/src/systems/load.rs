use std::collections::{HashMap, HashSet};

use crate::ecs::{LoadReport, PendingAsset, ViewerWorld};
use nightshade::ecs::prefab::GltfLoadResult;
use nightshade::prelude::*;
use protocol::{ClipInfo, ModelStats, MorphInfo, WorkerMessage};

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

/// Spawns the external agent's fetched glTF assets additively, leaving the
/// current scene in place. Each spawned model's entities are appended to the
/// tracked model so the scene tree and future despawns include them, then the
/// agent command is acknowledged.
#[cfg(feature = "agent")]
pub fn poll_agent_loads(viewer: &mut ViewerWorld, world: &mut World) {
    let loads = viewer
        .resources
        .incoming
        .agent_loads
        .lock()
        .ok()
        .map(|mut queue| std::mem::take(&mut *queue))
        .unwrap_or_default();
    for (correlation_id, bytes) in loads {
        match import_gltf_from_bytes(&bytes) {
            Ok(result) => {
                let roots = spawn_additive(viewer, world, result);
                crate::agent::ack_load(correlation_id, &roots);
            }
            Err(error) => {
                crate::agent::fail(correlation_id, &format!("import failed: {error}"));
            }
        }
    }
}

/// Spawns the external agent's fetched multi-file models (Polyhaven) additively,
/// reporting their root handles.
#[cfg(feature = "agent")]
pub fn poll_agent_models(viewer: &mut ViewerWorld, world: &mut World) {
    let models = viewer
        .resources
        .incoming
        .agent_models
        .lock()
        .ok()
        .map(|mut queue| std::mem::take(&mut *queue))
        .unwrap_or_default();
    for model in models {
        match nightshade::ecs::prefab::import_gltf_with_resources(&model.gltf, &model.resources) {
            Ok(result) => {
                let roots = spawn_additive(viewer, world, result);
                crate::agent::ack_load(model.correlation_id, &roots);
            }
            Err(error) => {
                crate::agent::fail(model.correlation_id, &format!("import failed: {error}"));
            }
        }
    }
}

/// Loads the external agent's fetched HDRIs as the skybox, acknowledging each.
#[cfg(feature = "agent")]
pub fn poll_agent_hdris(viewer: &mut ViewerWorld, world: &mut World) {
    let hdris = viewer
        .resources
        .incoming
        .agent_hdris
        .lock()
        .ok()
        .map(|mut queue| std::mem::take(&mut *queue))
        .unwrap_or_default();
    for (correlation_id, bytes) in hdris {
        load_hdr_skybox(world, bytes);
        crate::agent::ack_hdri(correlation_id);
    }
}

/// Imports and spawns a model without despawning the current scene, appending
/// the spawned entities to the tracked model. Returns the spawned root entities.
#[cfg(feature = "agent")]
fn spawn_additive(
    viewer: &mut ViewerWorld,
    world: &mut World,
    mut result: GltfLoadResult,
) -> Vec<Entity> {
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
        return roots;
    }

    let spawned: HashSet<u32> = world
        .core
        .query_entities(LOCAL_TRANSFORM)
        .map(|entity| entity.id)
        .filter(|id| !before.contains(id))
        .collect();
    let entities = ordered_entities(world, &roots, &spawned);

    viewer.resources.model.roots.extend(roots.iter().copied());
    viewer.resources.model.entities.extend(entities);
    viewer.resources.scene_sync.needs_tree = true;

    // Force the child cache to rebuild so a later transform edit on a root
    // propagates to the newly spawned descendants.
    world.resources.transform_state.children_cache_valid = false;
    world.resources.render_settings.color_grading.exposure = result.suggested_exposure;
    roots
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

    let mesh_count = result.meshes.len() as u32;
    let vertex_count = result
        .meshes
        .values()
        .map(|mesh| mesh.vertices.len() as u32)
        .sum();
    let triangle_count = result
        .meshes
        .values()
        .map(|mesh| (mesh.indices.len() / 3) as u32)
        .sum();
    let material_count = result.materials.len() as u32;
    let texture_count = result.texture_plan.len() as u32;

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

    world.resources.render_settings.color_grading.exposure = result.suggested_exposure;
    let stats = ModelStats {
        meshes: mesh_count,
        vertices: vertex_count,
        triangles: triangle_count,
        materials: material_count,
        textures: texture_count,
        dimensions: [0.0, 0.0, 0.0],
    };
    let clips = result
        .animations
        .iter()
        .map(|clip| ClipInfo {
            name: clip.name.clone(),
            duration: clip.duration,
        })
        .collect();
    let variants = result
        .prefabs
        .first()
        .map(|prefab| prefab.material_variants.clone())
        .unwrap_or_default();
    let morphs = morph_meshes(world, &viewer.resources.model.entities);
    viewer.resources.model.report = Some(LoadReport {
        stats,
        clips,
        variants,
        morphs,
        exposure: result.suggested_exposure,
        delay: 1,
    });
}

/// Sends the load report a frame after spawn, once global transforms have
/// updated so the model dimensions are correct.
pub fn flush_report(viewer: &mut ViewerWorld, world: &World) {
    let delay = match &viewer.resources.model.report {
        Some(report) => report.delay,
        None => return,
    };
    if delay > 0 {
        if let Some(report) = viewer.resources.model.report.as_mut() {
            report.delay -= 1;
        }
        return;
    }
    let dimensions = crate::systems::camera::model_bounds(viewer, world)
        .map(|(min, max)| {
            let size = max - min;
            [size.x, size.y, size.z]
        })
        .unwrap_or([0.0, 0.0, 0.0]);
    let mut report = viewer.resources.model.report.take().unwrap();
    report.stats.dimensions = dimensions;
    crate::post(&WorkerMessage::Loaded {
        stats: report.stats,
        clips: report.clips,
        variants: report.variants,
        morphs: report.morphs,
        exposure: report.exposure,
    });
}

/// Lists the spawned meshes that carry morph targets, with their current weights.
fn morph_meshes(world: &World, entities: &[Entity]) -> Vec<MorphInfo> {
    entities
        .iter()
        .filter_map(|&entity| {
            let weights = world.core.get_morph_weights(entity)?;
            let name = world
                .core
                .get_name(entity)
                .map(|name| name.0.clone())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| weights.mesh_name.clone());
            Some(MorphInfo {
                id: entity.id,
                name,
                weights: weights.weights.clone(),
            })
        })
        .collect()
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
pub(crate) fn despawn_current(viewer: &mut ViewerWorld, world: &mut World) {
    let camera = world.resources.active_camera;
    for entity in std::mem::take(&mut viewer.resources.model.entities) {
        if Some(entity) == camera {
            continue;
        }
        if world.core.entity_has_components(entity, LOCAL_TRANSFORM) {
            despawn_recursive_immediate(world, entity);
        }
    }
    viewer.resources.model.roots.clear();
    world.resources.mesh_render_state.request_full_rebuild();
}
