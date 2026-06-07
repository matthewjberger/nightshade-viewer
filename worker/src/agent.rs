use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use nightshade::ecs::transform::components::{LocalTransform, Parent};
use nightshade::prelude::serde_json::{self, Value};
use nightshade::prelude::{ehttp, *};
use protocol::{
    AgentCommand, AgentRequest, AgentResponse, ComponentInfo, CorrelationId, Delta, DeltaBatch,
    EntityRef, Environment, GetResult, Snapshot, SnapshotEntity, SubscriptionFilter,
    SubscriptionId, Version, WorkerMessage, WritePolicyInfo,
};

use crate::ecs::FetchState;
use crate::post;
use crate::state::Viewer;

/// Whether the generic component bag may carry a component, mirrored from the
/// protocol but kept as a cheap `Copy` value for the registry.
#[derive(Clone, Copy)]
enum Policy {
    Free,
    Owned(&'static str),
    Derived,
}

/// One registry entry: name, mask, policy, and the type-driven closures. Schema
/// and example both flow through the same `serde` impl so they cannot drift.
#[derive(Clone, Copy)]
struct ComponentEntry {
    name: &'static str,
    mask: u64,
    policy: Policy,
    serialize: fn(&World, Entity) -> Option<Value>,
    deserialize: fn(&mut World, Entity, &Value) -> Result<(), String>,
    sample: fn() -> Value,
    collect_changed: fn(&mut World, &mut dyn FnMut(Entity, Value)),
}

fn no_cascade(_world: &mut World, _entity: Entity) {}

macro_rules! entry {
    ($name:literal, $field:ident, $get:ident, $set:ident, $mask:ident, $ty:ty, $policy:expr, $cascade:expr) => {
        ComponentEntry {
            name: $name,
            mask: $mask,
            policy: $policy,
            serialize: |world, entity| {
                world
                    .core
                    .$get(entity)
                    .and_then(|value| serde_json::to_value(value).ok())
            },
            deserialize: |world, entity, value| {
                let parsed: $ty =
                    serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
                world.core.$set(entity, parsed);
                $cascade(world, entity);
                Ok(())
            },
            sample: || serde_json::to_value(<$ty>::default()).unwrap_or(Value::Null),
            collect_changed: |world, callback| {
                world
                    .core
                    .for_each_mut_changed($mask, 0, |entity, arrays, index| {
                        if let Ok(value) = serde_json::to_value(&arrays.$field[index]) {
                            callback(entity, value);
                        }
                    });
            },
        }
    };
}

fn registry() -> Vec<ComponentEntry> {
    use nightshade::ecs::bounding_volume::components::BoundingVolume;
    use nightshade::ecs::camera::components::Camera;
    use nightshade::ecs::light::components::Light;
    use nightshade::ecs::material::components::MaterialRef;
    use nightshade::ecs::primitives::{CastsShadow, Name, RenderLayer, Visibility};

    vec![
        entry!(
            "local_transform",
            local_transform,
            get_local_transform,
            set_local_transform,
            LOCAL_TRANSFORM,
            LocalTransform,
            Policy::Free,
            mark_local_transform_dirty
        ),
        entry!(
            "parent",
            parent,
            get_parent,
            set_parent,
            PARENT,
            Parent,
            Policy::Owned("reparent"),
            no_cascade
        ),
        entry!(
            "name",
            name,
            get_name,
            set_name,
            NAME,
            Name,
            Policy::Free,
            no_cascade
        ),
        entry!(
            "visibility",
            visibility,
            get_visibility,
            set_visibility,
            VISIBILITY,
            Visibility,
            Policy::Free,
            no_cascade
        ),
        entry!(
            "camera",
            camera,
            get_camera,
            set_camera,
            CAMERA,
            Camera,
            Policy::Free,
            no_cascade
        ),
        entry!(
            "light",
            light,
            get_light,
            set_light,
            LIGHT,
            Light,
            Policy::Free,
            no_cascade
        ),
        entry!(
            "material_ref",
            material_ref,
            get_material_ref,
            set_material_ref,
            MATERIAL_REF,
            MaterialRef,
            Policy::Free,
            no_cascade
        ),
        entry!(
            "bounding_volume",
            bounding_volume,
            get_bounding_volume,
            set_bounding_volume,
            BOUNDING_VOLUME,
            BoundingVolume,
            Policy::Derived,
            no_cascade
        ),
        entry!(
            "casts_shadow",
            casts_shadow,
            get_casts_shadow,
            set_casts_shadow,
            CASTS_SHADOW,
            CastsShadow,
            Policy::Free,
            no_cascade
        ),
        entry!(
            "render_layer",
            render_layer,
            get_render_layer,
            set_render_layer,
            RENDER_LAYER,
            RenderLayer,
            Policy::Free,
            no_cascade
        ),
    ]
}

/// Per-worker agent state. Lives in a `thread_local` because the collection and
/// apply systems are `fn(&mut World)` and cannot carry it.
#[derive(Default)]
struct AgentState {
    inbound: Vec<(CorrelationId, AgentCommand)>,
    pending_subscribes: Vec<(CorrelationId, SubscriptionFilter)>,
    subscriptions: HashMap<SubscriptionId, SubscriptionFilter>,
    next_subscription_id: SubscriptionId,
    version: Version,
    shadow: HashMap<Entity, u64>,
    baseline_set: bool,
    applied_this_frame: Vec<CorrelationId>,
}

thread_local! {
    static AGENT: RefCell<AgentState> = RefCell::new(AgentState {
        next_subscription_id: 1,
        ..AgentState::default()
    });
}

/// Inserts the agent systems into the engine frame schedule. Apply runs before
/// transform propagation so cascades resolve the same frame; collection runs
/// last so it sees every mutation in this frame's change-detection window.
pub fn install_systems(world: &mut World) {
    schedule_insert_before(
        &mut world.resources.schedules.frame,
        system_names::TRANSFORM_SYSTEMS,
        "agent_apply",
        agent_apply_system,
    );
    schedule_push(
        &mut world.resources.schedules.frame,
        "agent_collect",
        agent_collect_system,
    );
}

/// Routes one agent request. Reads answer immediately from the idle world;
/// mutating commands queue for the apply system; select and load need the
/// viewer resources and run here.
pub fn handle_agent_request(world: &mut World, viewer: &mut Viewer, request: AgentRequest) {
    match request {
        AgentRequest::ListComponentTypes { correlation_id } => {
            let components = registry().iter().map(component_info).collect();
            post(&WorkerMessage::Agent(AgentResponse::ComponentTypes {
                correlation_id,
                components,
            }));
        }
        AgentRequest::Query {
            correlation_id,
            component_types,
        } => {
            let mask = mask_for(&component_types);
            let entities = world
                .core
                .get_all_entities()
                .into_iter()
                .filter(|entity| world.core.component_mask(*entity).unwrap_or(0) & mask == mask)
                .map(to_ref)
                .collect();
            post(&WorkerMessage::Agent(AgentResponse::QueryResult {
                correlation_id,
                entities,
            }));
        }
        AgentRequest::GetComponents {
            correlation_id,
            entity,
            component_types,
        } => {
            let result = match live(world, entity) {
                Some(handle) => {
                    let registry = registry();
                    let components = component_types
                        .iter()
                        .filter_map(|name| {
                            registry
                                .iter()
                                .find(|entry| entry.name == name)
                                .and_then(|entry| (entry.serialize)(world, handle))
                                .map(|value| (name.clone(), value))
                        })
                        .collect();
                    GetResult::Live { entity, components }
                }
                None => GetResult::NotLive { entity },
            };
            post(&WorkerMessage::Agent(AgentResponse::GetResult {
                correlation_id,
                result,
            }));
        }
        AgentRequest::Command {
            correlation_id,
            command,
        } => handle_command(world, viewer, correlation_id, command),
        AgentRequest::Subscribe {
            correlation_id,
            filter,
        } => AGENT.with(|agent| {
            agent
                .borrow_mut()
                .pending_subscribes
                .push((correlation_id, filter));
        }),
        AgentRequest::Unsubscribe {
            correlation_id,
            subscription_id,
        } => {
            AGENT.with(|agent| {
                agent.borrow_mut().subscriptions.remove(&subscription_id);
            });
            post(&WorkerMessage::Agent(AgentResponse::Unsubscribed {
                correlation_id,
                subscription_id,
            }));
        }
        AgentRequest::ViewerAction {
            correlation_id,
            message,
        } => {
            crate::apply_client_message(world, viewer, *message);
            ack(correlation_id, current_version());
        }
        AgentRequest::GetViewerState { correlation_id } => {
            let state = build_viewer_state(world, viewer);
            post(&WorkerMessage::Agent(AgentResponse::ViewerState {
                correlation_id,
                state,
            }));
        }
        AgentRequest::SetEnvironment {
            correlation_id,
            environment,
        } => apply_environment(world, viewer, environment, correlation_id),
        AgentRequest::Resync { .. } => {}
    }
}

fn handle_command(
    world: &mut World,
    viewer: &mut Viewer,
    correlation_id: CorrelationId,
    command: AgentCommand,
) {
    match command {
        AgentCommand::SelectNode { entity } => match live(world, entity) {
            Some(handle) => {
                crate::systems::selection::select(&mut viewer.viewer, world, Some(handle));
                post(&WorkerMessage::Agent(AgentResponse::CommandApplied {
                    correlation_id,
                    version: current_version(),
                }));
            }
            None => fail(correlation_id, "entity not live"),
        },
        AgentCommand::LoadGltf { uri } => start_load(viewer, &uri, correlation_id),
        other => AGENT.with(|agent| agent.borrow_mut().inbound.push((correlation_id, other))),
    }
}

fn start_load(viewer: &mut Viewer, uri: &str, correlation_id: CorrelationId) {
    post(&WorkerMessage::Agent(AgentResponse::CommandProgress {
        correlation_id,
        stage: format!("fetching {uri}"),
    }));
    let queue = viewer.viewer.resources.incoming.agent_loads.clone();
    ehttp::fetch(ehttp::Request::get(uri), move |result| match result {
        Ok(response) if response.ok => {
            if let Ok(mut guard) = queue.lock() {
                guard.push((correlation_id, response.bytes));
            }
        }
        Ok(response) => fail(
            correlation_id,
            &format!("fetch failed: {}", response.status),
        ),
        Err(error) => fail(correlation_id, &format!("fetch error: {error}")),
    });
}

/// Acknowledges an additive agent load once the model has spawned, reporting the
/// spawned root handles so the agent can position the model by its root. Called
/// from the load poll, which holds the viewer the apply systems cannot see.
pub fn ack_load(correlation_id: CorrelationId, roots: &[Entity]) {
    post(&WorkerMessage::Agent(AgentResponse::Loaded {
        correlation_id,
        version: current_version(),
        roots: roots.iter().map(|entity| to_ref(*entity)).collect(),
    }));
}

fn agent_apply_system(world: &mut World) {
    let commands = AGENT.with(|agent| std::mem::take(&mut agent.borrow_mut().inbound));
    if commands.is_empty() {
        return;
    }
    let registry = registry();
    for (correlation_id, command) in commands {
        match apply_command(world, &registry, command) {
            Ok(()) => {
                AGENT.with(|agent| agent.borrow_mut().applied_this_frame.push(correlation_id))
            }
            Err(error) => fail(correlation_id, &error),
        }
    }
}

fn apply_command(
    world: &mut World,
    registry: &[ComponentEntry],
    command: AgentCommand,
) -> Result<(), String> {
    match command {
        AgentCommand::SetComponents { entity, components } => {
            let handle = live(world, entity).ok_or("entity not live")?;
            for (name, _) in &components {
                writable(registry, name)?;
            }
            for (name, value) in &components {
                let entry = find(registry, name)?;
                (entry.deserialize)(world, handle, value)?;
            }
            Ok(())
        }
        AgentCommand::SpawnEntity { components } => {
            let mut mask = 0u64;
            for (name, _) in &components {
                mask |= writable(registry, name)?;
            }
            if mask & LOCAL_TRANSFORM != 0 {
                mask |= GLOBAL_TRANSFORM | LOCAL_TRANSFORM_DIRTY;
            }
            let handle = nightshade::ecs::world::commands::spawn_entities(world, mask, 1)
                .into_iter()
                .next()
                .ok_or("spawn failed")?;
            for (name, value) in &components {
                let entry = find(registry, name)?;
                (entry.deserialize)(world, handle, value)?;
            }
            Ok(())
        }
        AgentCommand::RemoveComponents {
            entity,
            component_types,
        } => {
            let handle = live(world, entity).ok_or("entity not live")?;
            let mut mask = 0u64;
            for name in &component_types {
                mask |= writable(registry, name)?;
            }
            world.core.remove_components(handle, mask);
            Ok(())
        }
        AgentCommand::Reparent { child, new_parent } => {
            let handle = live(world, child).ok_or("child not live")?;
            let parent = match new_parent {
                Some(reference) => Some(Parent(Some(
                    live(world, reference).ok_or("new parent not live")?,
                ))),
                None => None,
            };
            nightshade::ecs::transform::systems::update_parent(world, handle, parent);
            Ok(())
        }
        AgentCommand::DeleteEntity { entity } => {
            let handle = live(world, entity).ok_or("entity not live")?;
            nightshade::ecs::world::commands::despawn_recursive_immediate(world, handle);
            Ok(())
        }
        AgentCommand::SelectNode { .. } | AgentCommand::LoadGltf { .. } => {
            Err("command handled out of band".to_string())
        }
    }
}

fn agent_collect_system(world: &mut World) {
    AGENT.with(|agent| {
        let mut state = agent.borrow_mut();
        let pending = std::mem::take(&mut state.pending_subscribes);
        let applied = std::mem::take(&mut state.applied_this_frame);

        if state.subscriptions.is_empty() && pending.is_empty() {
            let version = state.version;
            drop(state);
            for correlation_id in applied {
                ack(correlation_id, version);
            }
            return;
        }

        let registry = registry();
        let old_version = state.version;
        let new_version = old_version + 1;

        for (correlation_id, filter) in pending {
            let subscription_id = state.next_subscription_id;
            state.next_subscription_id += 1;
            let snapshot = build_snapshot(world, &filter, &registry, new_version);
            state.subscriptions.insert(subscription_id, filter);
            post(&WorkerMessage::Agent(AgentResponse::Subscribed {
                correlation_id,
                subscription_id,
                snapshot,
            }));
        }

        let tracked = tracked_entries(&state.subscriptions, &registry);
        let current = current_masks(world);
        let deltas = if state.baseline_set {
            compute_deltas(world, &state.shadow, &current, &tracked)
        } else {
            Vec::new()
        };

        post(&WorkerMessage::Agent(AgentResponse::Batch {
            batch: DeltaBatch {
                base_version: old_version,
                target_version: new_version,
                deltas,
                checksum: None,
            },
        }));

        state.shadow = current;
        state.baseline_set = true;
        state.version = new_version;
        for correlation_id in applied {
            ack(correlation_id, new_version);
        }
    });
}

fn compute_deltas(
    world: &mut World,
    shadow: &HashMap<Entity, u64>,
    current: &HashMap<Entity, u64>,
    tracked: &[ComponentEntry],
) -> Vec<Delta> {
    let mut creates = Vec::new();
    let mut values = Vec::new();
    let mut deletes = Vec::new();
    let mut spawned = HashSet::new();
    let mut added_pairs = HashSet::new();

    for (entity, mask) in current {
        if shadow.contains_key(entity) {
            continue;
        }
        spawned.insert(*entity);
        let components = tracked
            .iter()
            .filter(|entry| mask & entry.mask != 0)
            .filter_map(|entry| {
                (entry.serialize)(world, *entity).map(|value| (entry.name.to_string(), value))
            })
            .collect();
        creates.push(Delta::Spawned {
            entity: to_ref(*entity),
            components,
            origin: None,
        });
    }

    for (entity, mask) in current {
        let Some(old_mask) = shadow.get(entity) else {
            continue;
        };
        let added = mask & !old_mask;
        let removed = old_mask & !mask;
        for entry in tracked {
            if added & entry.mask != 0 {
                if let Some(value) = (entry.serialize)(world, *entity) {
                    creates.push(Delta::Added {
                        entity: to_ref(*entity),
                        component: entry.name.to_string(),
                        value,
                        origin: None,
                    });
                }
                added_pairs.insert((*entity, entry.mask));
            }
            if removed & entry.mask != 0 {
                deletes.push(Delta::Removed {
                    entity: to_ref(*entity),
                    component: entry.name.to_string(),
                    origin: None,
                });
            }
        }
    }

    for entry in tracked {
        let mut hits = Vec::new();
        (entry.collect_changed)(world, &mut |entity, value| hits.push((entity, value)));
        for (entity, value) in hits {
            if spawned.contains(&entity) || added_pairs.contains(&(entity, entry.mask)) {
                continue;
            }
            if current.contains_key(&entity) {
                values.push(Delta::Changed {
                    entity: to_ref(entity),
                    component: entry.name.to_string(),
                    value,
                    origin: None,
                });
            }
        }
    }

    for entity in shadow.keys() {
        if !current.contains_key(entity) {
            deletes.push(Delta::Despawned {
                entity: to_ref(*entity),
                origin: None,
            });
        }
    }

    creates.extend(values);
    creates.extend(deletes);
    creates
}

fn build_snapshot(
    world: &World,
    filter: &SubscriptionFilter,
    registry: &[ComponentEntry],
    version: Version,
) -> Snapshot {
    let tracked = filter_entries(filter, registry);
    let candidates: Vec<Entity> = match &filter.entities {
        Some(references) => references
            .iter()
            .filter_map(|reference| live(world, *reference))
            .collect(),
        None => world.core.get_all_entities(),
    };
    let entities = candidates
        .into_iter()
        .filter_map(|entity| {
            let mask = world.core.component_mask(entity).unwrap_or(0);
            let components: Vec<(String, Value)> = tracked
                .iter()
                .filter(|entry| mask & entry.mask != 0)
                .filter_map(|entry| {
                    (entry.serialize)(world, entity).map(|value| (entry.name.to_string(), value))
                })
                .collect();
            (!components.is_empty()).then(|| SnapshotEntity {
                entity: to_ref(entity),
                components,
            })
        })
        .collect();
    Snapshot { version, entities }
}

fn current_masks(world: &World) -> HashMap<Entity, u64> {
    world
        .core
        .get_all_entities()
        .into_iter()
        .map(|entity| (entity, world.core.component_mask(entity).unwrap_or(0)))
        .collect()
}

fn tracked_entries(
    subscriptions: &HashMap<SubscriptionId, SubscriptionFilter>,
    registry: &[ComponentEntry],
) -> Vec<ComponentEntry> {
    let mut names: HashSet<&str> = HashSet::new();
    let mut all = false;
    for filter in subscriptions.values() {
        if filter.component_types.is_empty() {
            all = true;
        }
        for name in &filter.component_types {
            names.insert(name.as_str());
        }
    }
    registry
        .iter()
        .filter(|entry| all || names.contains(entry.name))
        .copied()
        .collect()
}

fn filter_entries(filter: &SubscriptionFilter, registry: &[ComponentEntry]) -> Vec<ComponentEntry> {
    if filter.component_types.is_empty() {
        return registry.to_vec();
    }
    registry
        .iter()
        .filter(|entry| filter.component_types.iter().any(|name| name == entry.name))
        .copied()
        .collect()
}

fn component_info(entry: &ComponentEntry) -> ComponentInfo {
    let example = (entry.sample)();
    let schema = shape_of(&example);
    ComponentInfo {
        name: entry.name.to_string(),
        write_policy: match entry.policy {
            Policy::Free => WritePolicyInfo::Free,
            Policy::Owned(command) => WritePolicyInfo::Owned {
                command: command.to_string(),
            },
            Policy::Derived => WritePolicyInfo::Derived,
        },
        schema,
        example,
    }
}

fn shape_of(value: &Value) -> Value {
    match value {
        Value::Null => Value::String("null".to_string()),
        Value::Bool(_) => Value::String("boolean".to_string()),
        Value::Number(_) => Value::String("number".to_string()),
        Value::String(_) => Value::String("string".to_string()),
        Value::Array(items) => {
            let inner = items
                .first()
                .map(shape_of)
                .unwrap_or(Value::String("any".to_string()));
            Value::Array(vec![inner])
        }
        Value::Object(map) => {
            let shaped = map
                .iter()
                .map(|(key, inner)| (key.clone(), shape_of(inner)))
                .collect();
            Value::Object(shaped)
        }
    }
}

fn mask_for(component_types: &[String]) -> u64 {
    let registry = registry();
    component_types
        .iter()
        .filter_map(|name| registry.iter().find(|entry| entry.name == name))
        .fold(0u64, |mask, entry| mask | entry.mask)
}

fn find<'registry>(
    registry: &'registry [ComponentEntry],
    name: &str,
) -> Result<&'registry ComponentEntry, String> {
    registry
        .iter()
        .find(|entry| entry.name == name)
        .ok_or_else(|| format!("unknown component: {name}"))
}

fn writable(registry: &[ComponentEntry], name: &str) -> Result<u64, String> {
    let entry = find(registry, name)?;
    match entry.policy {
        Policy::Free => Ok(entry.mask),
        Policy::Owned(command) => Err(format!("{name} is owned by the {command} command")),
        Policy::Derived => Err(format!("{name} is derived and cannot be written")),
    }
}

fn live(world: &World, reference: EntityRef) -> Option<Entity> {
    let entity = Entity {
        id: reference.id,
        generation: reference.generation,
    };
    world.core.component_mask(entity).map(|_| entity)
}

fn to_ref(entity: Entity) -> EntityRef {
    EntityRef {
        id: entity.id,
        generation: entity.generation,
    }
}

fn current_version() -> Version {
    AGENT.with(|agent| agent.borrow().version)
}

fn ack(correlation_id: CorrelationId, version: Version) {
    post(&WorkerMessage::Agent(AgentResponse::CommandApplied {
        correlation_id,
        version,
    }));
}

pub fn fail(correlation_id: CorrelationId, error: &str) {
    post(&WorkerMessage::Agent(AgentResponse::CommandFailed {
        correlation_id,
        error: error.to_string(),
    }));
}

/// Acknowledges an agent HDRI load once the skybox has been queued. Called from
/// the load poll, which holds the viewer the apply systems cannot see.
pub fn ack_hdri(correlation_id: CorrelationId) {
    ack(correlation_id, current_version());
}

fn build_viewer_state(world: &World, viewer: &Viewer) -> Value {
    let settings = &world.resources.render_settings;
    let debug = &world.resources.debug_draw;
    let selection = viewer
        .viewer
        .resources
        .selection
        .selected
        .map(|entity| serde_json::to_value(to_ref(entity)).unwrap_or(Value::Null));
    serde_json::json!({
        "fps": world.resources.window.timing.frames_per_second,
        "render": {
            "atmosphere": format!("{:?}", settings.atmosphere),
            "show_sky": settings.show_sky,
            "clear_color": settings.clear_color,
            "exposure": settings.color_grading.exposure,
            "tonemap": format!("{:?}", settings.color_grading.tonemap_algorithm),
            "show_grid": debug.show_grid,
            "show_normals": debug.show_normals,
            "show_bounds": debug.show_bounding_volumes,
            "pbr_debug": format!("{:?}", debug.pbr_debug_mode),
        },
        "model": {
            "roots": viewer.viewer.resources.model.roots.len(),
            "entities": viewer.viewer.resources.model.entities.len(),
        },
        "selected": selection,
        "assets": {
            "khronos": khronos_list(viewer),
            "hdris": polyhaven_list(&viewer.viewer.resources.browsers.hdris),
            "models": polyhaven_list(&viewer.viewer.resources.browsers.models),
        },
    })
}

fn khronos_list(viewer: &Viewer) -> Value {
    let Ok(state) = viewer.viewer.resources.browsers.khronos.lock() else {
        return Value::Null;
    };
    match &*state {
        FetchState::Loaded(entries) => Value::Array(
            entries
                .iter()
                .map(|asset| {
                    serde_json::json!({
                        "name": asset.name,
                        "label": asset.label,
                        "glb_url": asset.glb_url,
                    })
                })
                .collect(),
        ),
        FetchState::Loading => Value::String("loading".to_string()),
        FetchState::Failed => Value::String("failed".to_string()),
        FetchState::Idle => Value::String("idle (call refresh_browsers)".to_string()),
    }
}

fn polyhaven_list(
    handle: &std::sync::Arc<std::sync::Mutex<FetchState<Vec<crate::ecs::PolyAsset>>>>,
) -> Value {
    let Ok(state) = handle.lock() else {
        return Value::Null;
    };
    match &*state {
        FetchState::Loaded(entries) => Value::Array(
            entries
                .iter()
                .map(|asset| serde_json::json!({ "slug": asset.slug, "name": asset.name }))
                .collect(),
        ),
        FetchState::Loading => Value::String("loading".to_string()),
        FetchState::Failed => Value::String("failed".to_string()),
        FetchState::Idle => Value::String("idle (call refresh_browsers)".to_string()),
    }
}

fn apply_environment(
    world: &mut World,
    viewer: &mut Viewer,
    environment: Environment,
    correlation_id: CorrelationId,
) {
    if let Some(show) = environment.show_sky {
        world.resources.render_settings.show_sky = show;
    }
    if let Some(color) = environment.clear_color {
        world.resources.render_settings.clear_color = color;
    }
    if let Some(exposure) = environment.exposure {
        world.resources.render_settings.color_grading.exposure = exposure;
    }
    if let Some(hour) = environment.hour {
        world.resources.renderer_state.day_night.hour = hour;
    }
    if let Some(name) = &environment.atmosphere {
        let Some(atmosphere) = parse_atmosphere(name) else {
            fail(correlation_id, &format!("unknown atmosphere: {name}"));
            return;
        };
        world.resources.render_settings.atmosphere = atmosphere;
        if is_procedural(atmosphere) {
            let hour = world.resources.renderer_state.day_night.hour;
            capture_procedural_atmosphere_ibl(world, atmosphere, hour);
        }
    }
    if let Some(uri) = environment.hdri_uri {
        world.resources.render_settings.atmosphere = Atmosphere::Hdr;
        start_hdri(viewer, &uri, correlation_id);
        return;
    }
    ack(correlation_id, current_version());
}

fn start_hdri(viewer: &mut Viewer, uri: &str, correlation_id: CorrelationId) {
    post(&WorkerMessage::Agent(AgentResponse::CommandProgress {
        correlation_id,
        stage: format!("fetching {uri}"),
    }));
    let queue = viewer.viewer.resources.incoming.agent_hdris.clone();
    ehttp::fetch(ehttp::Request::get(uri), move |result| match result {
        Ok(response) if response.ok => {
            if let Ok(mut guard) = queue.lock() {
                guard.push((correlation_id, response.bytes));
            }
        }
        Ok(response) => fail(
            correlation_id,
            &format!("fetch failed: {}", response.status),
        ),
        Err(error) => fail(correlation_id, &format!("fetch error: {error}")),
    });
}

fn parse_atmosphere(name: &str) -> Option<Atmosphere> {
    match name.to_lowercase().replace(['_', ' ', '-'], "").as_str() {
        "none" => Some(Atmosphere::None),
        "sky" => Some(Atmosphere::Sky),
        "cloudysky" | "cloudy" => Some(Atmosphere::CloudySky),
        "space" => Some(Atmosphere::Space),
        "nebula" => Some(Atmosphere::Nebula),
        "sunset" => Some(Atmosphere::Sunset),
        "daynight" => Some(Atmosphere::DayNight),
        "hdr" => Some(Atmosphere::Hdr),
        _ => None,
    }
}

fn is_procedural(atmosphere: Atmosphere) -> bool {
    matches!(
        atmosphere,
        Atmosphere::Sky
            | Atmosphere::CloudySky
            | Atmosphere::Space
            | Atmosphere::Nebula
            | Atmosphere::Sunset
            | Atmosphere::DayNight
    )
}
