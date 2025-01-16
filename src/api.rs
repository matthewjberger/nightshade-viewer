use crate::prelude::*;

/// Engine outputs
#[derive(Debug, Clone)]
pub enum EngineEvent {
    EntityCreated(EntityId),
}

/// Inputs to the engine
#[derive(Debug, Clone)]
pub enum Command {
    Entity(EntityCommand),
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

/// Queue a command to be executed by the command system
pub fn queue_command(context: &mut Context, command: Command) {
    context.resources.commands.push(command);
}

/// Push an event to be processed by event handlers
pub fn push_event(context: &mut Context, event: EngineEvent) {
    context.resources.events.push(event);
}

/// System that processes any pending commands in the command queue
pub fn execute_commands_system(context: &mut Context) {
    let commands = std::mem::take(&mut context.resources.commands);

    for command in commands {
        log::info!("[Command] Executing command: {command:?}");
        match command {
            Command::Entity(command) => {
                execute_entity_command(context, command);
            }
        }
    }
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

    push_event(context, EngineEvent::EntityCreated(entity));

    entity
}

/// System that processes any pending events in the event queue
pub fn process_events_system(context: &mut Context) {
    let events = std::mem::take(&mut context.resources.events);

    for event in events {
        match event {
            EngineEvent::EntityCreated(entity) => {
                if let Some(name) = get_component::<Name>(context, entity, NAME) {
                    log::info!("[Event] Entity created: {} ({})", name.0, entity);
                } else {
                    log::info!("[Event] Entity created: {}", entity);
                }
            }
        }
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

    push_event(context, EngineEvent::EntityCreated(entity));

    entity
}
