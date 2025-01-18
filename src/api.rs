use crate::paint::{paint_box, paint_entity, Painting};
use crate::prelude::*;
use serde::{Deserialize, Serialize};

/// Engine outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    EntityCreated {
        entity_id: EntityId,
        source: Option<u64>,
    },
    Report {
        report: Report,
        source: Option<u64>,
    },
    Rpc {
        event: RpcEvent,
        source: Option<u64>,
    },
    #[cfg(not(target_arch = "wasm32"))]
    Ipc {
        event: IpcEvent,
        source: Option<u64>,
    },
}

/// Emitted in response to queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Report {
    Cameras { entity_ids: Vec<EntityId> },
}

/// Inputs to the engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Entity { command: EntityCommand },
    Request { command: RequestCommand },
    Rpc { command: RpcCommand },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityCommand {
    SpawnCube {
        position: nalgebra_glm::Vec3,
        size: f32,
        name: String,
    },
    SpawnCamera {
        position: nalgebra_glm::Vec3,
        name: String,
    },
}

/// Commands that request information from the engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestCommand {
    RequestCameraEntities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMessage {
    pub id: u64,
    pub command: Command,
}

/// Add this to the Context resources
#[derive(Default)]
pub struct CommandState {
    pub next_command_id: u64,
}

/// Push a command to be executed by the command system
pub fn push_command(context: &mut Context, command: Command) {
    let command_id = context.resources.command_state.next_command_id;
    log::debug!("[Command] Executing command {}: {:?}", command_id, command);
    context.resources.command_state.next_command_id += 1;

    // Execute command immediately with current command ID
    match command {
        Command::Entity { command } => {
            execute_entity_command(context, command, command_id);
        }
        Command::Request { command } => {
            execute_query_command(context, command, command_id);
        }
        Command::Rpc { command } => {
            execute_rpc_command(context, command);
        }
    }
}

/// Push an event to be processed by event handlers
pub fn push_event(context: &mut Context, event: Event) {
    log::debug!("[Event] {:?}", event);
    context.resources.events.push(event);
}

/// System that processes any pending commands in the command queue
pub fn execute_commands_system(_context: &mut Context) {
    // This is now empty since we execute commands immediately in push_command
}

fn execute_query_command(context: &mut Context, command: RequestCommand, command_id: u64) {
    match command {
        RequestCommand::RequestCameraEntities => {
            report_cameras(context, command_id);
        }
    }
}

fn report_cameras(context: &mut Context, command_id: u64) {
    let cameras = query_entities(context, CAMERA);
    push_event_with_source(
        context,
        Event::Report {
            report: Report::Cameras {
                entity_ids: cameras,
            },
            source: Some(command_id),
        },
        command_id,
    );
}

fn execute_entity_command(context: &mut Context, command: EntityCommand, command_id: u64) {
    log::debug!(
        "[API] Executing entity command {}: {:?}",
        command_id,
        command
    );
    match command {
        EntityCommand::SpawnCube {
            position,
            size,
            name,
        } => {
            spawn_cube(context, position, size, name, command_id);
        }
        EntityCommand::SpawnCamera { position, name } => {
            spawn_camera(context, position, name, command_id);
        }
    }
}

fn spawn_cube(
    context: &mut Context,
    position: nalgebra_glm::Vec3,
    size: f32,
    name: String,
    command_id: u64,
) -> EntityId {
    log::debug!("[API] Spawning cube with command ID: {}", command_id);

    // Spawn entity with required components
    let entity = spawn_entities(
        context,
        LOCAL_TRANSFORM | GLOBAL_TRANSFORM | NAME | LINES | QUADS,
        1,
    )[0];

    // Set name
    if let Some(name_comp) = get_component_mut::<Name>(context, entity, NAME) {
        *name_comp = Name(name);
    }

    // Set transform
    if let Some(transform) = get_component_mut::<LocalTransform>(context, entity, LOCAL_TRANSFORM) {
        transform.translation = position;
        transform.scale = nalgebra_glm::vec3(1.0, 1.0, 1.0);
    }

    // Paint the cube
    let mut painting = Painting::default();
    paint_box(
        &mut painting,
        nalgebra_glm::vec3(0.0, 0.0, 0.0),
        nalgebra_glm::vec3(size, size, size),
        nalgebra_glm::vec4(1.0, 1.0, 1.0, 1.0),
    );
    paint_entity(context, entity, painting);

    push_event_with_source(
        context,
        Event::EntityCreated {
            entity_id: entity,
            source: Some(command_id),
        },
        command_id,
    );
    log::debug!(
        "[API] Spawned cube entity {} for command {}",
        entity.id,
        command_id
    );

    entity
}

/// Route events to the appropriate domains
pub fn route_events_system(context: &mut Context) {
    let events = std::mem::take(&mut context.resources.events);
    for event in events {
        log::debug!("[Event] {event:?}");
    }
}

fn spawn_camera(
    context: &mut Context,
    position: nalgebra_glm::Vec3,
    name: String,
    command_id: u64,
) -> EntityId {
    let entity = spawn_entities(
        context,
        LOCAL_TRANSFORM | GLOBAL_TRANSFORM | NAME | CAMERA,
        1,
    )[0];

    // Set name
    if let Some(name_comp) = get_component_mut::<Name>(context, entity, NAME) {
        *name_comp = Name(name);
    }

    // Set transform
    if let Some(transform) = get_component_mut::<LocalTransform>(context, entity, LOCAL_TRANSFORM) {
        transform.translation = position;
        transform.scale = nalgebra_glm::vec3(1.0, 1.0, 1.0);
    }

    // Make this the active camera if we don't have one
    if context.resources.active_camera_entity.is_none() {
        context.resources.active_camera_entity = Some(entity);
    }

    push_event_with_source(
        context,
        Event::EntityCreated {
            entity_id: entity,
            source: Some(command_id),
        },
        command_id,
    );

    entity
}

/// Add a helper function to create events with source IDs
pub fn push_event_with_source(context: &mut Context, event: Event, source: u64) {
    log::debug!("[API] Pushing event with source {}: {:?}", source, event);
    let event_with_source = match event {
        Event::EntityCreated { entity_id, .. } => Event::EntityCreated {
            entity_id,
            source: Some(source),
        },
        Event::Report { report, .. } => Event::Report {
            report,
            source: Some(source),
        },
        Event::Rpc { event, .. } => Event::Rpc {
            event,
            source: Some(source),
        },
        #[cfg(not(target_arch = "wasm32"))]
        Event::Ipc { event, .. } => Event::Ipc {
            event,
            source: Some(source),
        },
    };

    // First push to context
    push_event(context, event_with_source.clone());

    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(sender) = crate::ipc::EVENT_CHANNEL.0.try_lock() {
        log::debug!("[IPC] Sending event to channel: {:?}", event_with_source);
        if let Err(e) = sender.send(event_with_source) {
            log::error!("[API] Failed to send event to IPC channel: {}", e);
        } else {
            log::debug!("[IPC] Successfully sent event to channel");
        }
    } else {
        log::error!("[IPC] Failed to lock event channel sender");
    }
}
