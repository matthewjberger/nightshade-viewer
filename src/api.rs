use crate::prelude::*;

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
}

pub fn execute_command(context: &mut Context, command: Command) {
    match command {
        Command::Entity(EntityCommand::SpawnCube { position, size, name }) => {
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
            if let Some(transform) = get_component_mut::<LocalTransform>(context, entity, LOCAL_TRANSFORM)
            {
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
        }
    }
} 