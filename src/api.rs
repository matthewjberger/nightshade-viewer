use crate::prelude::*;

/// Engine outputs
#[derive(Debug, Clone)]
pub enum Event {
    EntityCreated { entity_id: EntityId },
    Report { report: Report },
    Rpc { event: RpcEvent },
}

/// Emitted in response to queries
#[derive(Debug, Clone)]
pub enum Report {
    Cameras { entity_ids: Vec<EntityId> },
}

/// Inputs to the engine
#[derive(Debug, Clone)]
pub enum Command {
    Entity { command: EntityCommand },
    Request { command: RequestCommand },
    Rpc { command: RpcCommand },
}

#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub enum RequestCommand {
    RequestCameraEntities,
}

/// Push a command to be executed by the command system
pub fn push_command(context: &mut Context, command: Command) {
    context.resources.commands.push(command);
}

/// Push an event to be processed by event handlers
pub fn push_event(context: &mut Context, event: Event) {
    context.resources.events.push(event);
}

/// System that processes any pending commands in the command queue
pub fn execute_commands_system(context: &mut Context) {
    let commands = std::mem::take(&mut context.resources.commands);

    for command in commands {
        log::info!("[Command] Executing command: {command:?}");
        match command {
            Command::Entity { command } => {
                execute_entity_command(context, command);
            }
            Command::Request { command } => {
                execute_query_command(context, command);
            }
            Command::Rpc { command } => {
                execute_rpc_command(context, command);
            }
        }
    }
}

fn execute_query_command(context: &mut Context, command: RequestCommand) {
    match command {
        RequestCommand::RequestCameraEntities => {
            report_cameras(context);
        }
    }
}

fn report_cameras(context: &mut Context) {
    let cameras = query_entities(context, CAMERA);
    push_event(
        context,
        Event::Report {
            report: Report::Cameras {
                entity_ids: cameras,
            },
        },
    );
}

fn execute_entity_command(context: &mut Context, command: EntityCommand) {
    match command {
        EntityCommand::SpawnCube {
            position,
            size,
            name,
        } => {
            spawn_cube(context, position, size, name);
        }
        EntityCommand::SpawnCamera { position, name } => {
            spawn_camera(context, position, name);
        }
    }
}

fn spawn_camera(context: &mut Context, position: nalgebra_glm::Vec3, name: String) -> EntityId {
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

    push_event(context, Event::EntityCreated { entity_id: entity });

    entity
}

/// Route events to the appropriate domains
pub fn route_events_system(context: &mut Context) {
    let events = std::mem::take(&mut context.resources.events);
    for event in events {
        log::info!("[Event] {event:?}");
    }
}

fn spawn_cube(
    context: &mut Context,
    position: nalgebra_glm::Vec3,
    size: f32,
    name: String,
) -> EntityId {
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

    push_event(context, Event::EntityCreated { entity_id: entity });

    entity
}
