use crate::{
    prelude::*,
    rpc::execute_rpc_command,
    ui::{format_timestamp, ApiLogEntry, ApiLogKind},
};
use enum2egui::{Gui, GuiInspect};
use enum2str::EnumStr;
use web_time::SystemTime;

// Commands - Input to engine
#[derive(Default, Debug, Clone, Gui, EnumStr)]
pub enum Command {
    #[default]
    Empty,
    Query {
        command: QueryCommand,
    },
    Spawn {
        command: SpawnCommand,
    },
    Rpc {
        command: RpcCommand,
    },
}

#[derive(Default, Debug, Clone, Gui, EnumStr)]
pub enum QueryCommand {
    #[default]
    Empty,
    ListCameras,
}

#[derive(Default, Debug, Clone, Gui, EnumStr)]
pub enum SpawnCommand {
    #[default]
    Empty,
    Cube {
        position: Vec3,
        size: f32,
        name: String,
    },
    Camera {
        position: Vec3,
        name: String,
    },
}

#[derive(Default, Debug, Clone, Gui)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<Vec3> for nalgebra_glm::Vec3 {
    fn from(val: Vec3) -> Self {
        nalgebra_glm::vec3(val.x, val.y, val.z)
    }
}

impl From<nalgebra_glm::Vec3> for Vec3 {
    fn from(val: nalgebra_glm::Vec3) -> Self {
        Self {
            x: val.x,
            y: val.y,
            z: val.z,
        }
    }
}

// Events - Output from engine
#[derive(Default, Debug, Clone, Gui, EnumStr)]
pub enum Event {
    #[default]
    None,
    EntityCreated {
        entity_id: EntityId,
    },
    CameraList {
        cameras: Vec<EntityId>,
    },
    WebsocketConnected,
    WebsocketDisconnected,
    WebsocketMessage {
        message: String,
    },
    WebsocketError {
        error: String,
    },
}

// Event storage in Resources
#[derive(Default)]
pub struct EventQueues {
    pub events: Vec<Event>,
}

// Public API - Just two functions
pub fn push_command(context: &mut Context, command: Command) {
    context.resources.commands.push(command);
}

pub fn push_event(context: &mut Context, event: Event) {
    context.resources.events.events.push(event);
}

// System for processing commands
pub fn execute_commands_system(context: &mut Context) {
    let commands = std::mem::take(&mut context.resources.commands);
    for command in commands {
        log::info!("[Command] {command:?}");
        // Add to API log with pre-computed timestamp
        let timestamp = SystemTime::now();
        context.resources.user_interface.api_log.push(ApiLogEntry {
            timestamp,
            timestamp_string: format_timestamp(timestamp),
            kind: ApiLogKind::Command,
            message: format!("{command:?}"),
        });
        execute_command(context, command);
    }
}

// System for processing events
pub fn process_events_system(context: &mut Context) {
    let events = std::mem::take(&mut context.resources.events.events);
    events.into_iter().for_each(|event| {
        log::info!("[Event] {event:?}");
        // Add to API log with pre-computed timestamp
        let timestamp = SystemTime::now();
        context.resources.user_interface.api_log.push(ApiLogEntry {
            timestamp,
            timestamp_string: format_timestamp(timestamp),
            kind: ApiLogKind::Event,
            message: format!("{event:?}"),
        });
    });
}

fn execute_command(context: &mut Context, command: Command) {
    match command {
        Command::Spawn { command } => execute_spawn_command(context, command),
        Command::Rpc { command } => {
            execute_rpc_command(context, command);
        }
        Command::Query { command } => execute_query_command(context, command),
        Command::Empty => {}
    }
}

fn execute_spawn_command(context: &mut Context, spawn_command: SpawnCommand) {
    match spawn_command {
        SpawnCommand::Empty => {}
        SpawnCommand::Cube {
            position,
            size,
            name,
        } => {
            let entity = spawn_cube(context, position.into(), size, name);
            push_event(context, Event::EntityCreated { entity_id: entity });
        }
        SpawnCommand::Camera { position, name } => {
            let entity = spawn_camera(context, position.into(), name);
            push_event(context, Event::EntityCreated { entity_id: entity });
        }
    }
}

fn execute_query_command(context: &mut Context, query_command: QueryCommand) {
    match query_command {
        QueryCommand::ListCameras => {
            let cameras = query_entities(context, CAMERA);
            push_event(context, Event::CameraList { cameras });
        }
        QueryCommand::Empty => {}
    }
}

// Helper functions
fn spawn_cube(
    context: &mut Context,
    position: nalgebra_glm::Vec3,
    size: f32,
    name: String,
) -> EntityId {
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

    entity
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

    entity
}
