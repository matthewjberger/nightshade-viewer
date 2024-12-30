crate::ecs! {
    Context {
        local_transform: LocalTransform => LOCAL_TRANSFORM,
        global_transform: GlobalTransform => GLOBAL_TRANSFORM,
        name: GlobalTransform => NAME,
        parent: Parent => PARENT,
        visible: Visible => VISIBLE,
    }
    Resources {
        #[serde(skip)] window: crate::modules::window::Window,
        #[serde(skip)] graphics: crate::modules::graphics::Graphics,
        #[serde(skip)] input: crate::modules::input::Input,
        #[serde(skip)] frame_timing: crate::modules::window::FrameTiming,
        #[serde(skip)] user_interface: crate::modules::ui::UserInterface,
    }
}

#[derive(Debug, Copy, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct LocalTransform {
    pub translation: nalgebra_glm::Vec3,
    pub rotation: nalgebra_glm::Quat,
    pub scale: nalgebra_glm::Vec3,
}

impl Default for LocalTransform {
    fn default() -> Self {
        Self {
            translation: nalgebra_glm::Vec3::new(0.0, 0.0, 0.0),
            rotation: nalgebra_glm::Quat::identity(),
            scale: nalgebra_glm::Vec3::new(1.0, 1.0, 1.0),
        }
    }
}

#[derive(Default, Debug, Copy, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GlobalTransform(pub nalgebra_glm::Mat4);

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Name(pub String);

#[derive(Default, Debug, Copy, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Parent(pub crate::modules::scene::EntityId);

#[derive(Default, Debug, Copy, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Visible;

pub mod queries {
    /// Queries for root nodes by looking for entities that do not have a Parent component
    pub fn query_root_nodes(
        context: &crate::modules::scene::Context,
    ) -> Vec<crate::modules::scene::EntityId> {
        let mut root_entities: Vec<crate::modules::scene::EntityId> = context
            .tables
            .iter()
            .filter_map(|table| {
                if crate::has_components!(table, crate::modules::scene::PARENT) {
                    return None;
                }
                Some(table.entity_indices.to_vec())
            })
            .flatten()
            .collect();
        root_entities.dedup();
        root_entities
    }

    // Query for the child entities of an entity
    pub fn query_children(
        context: &crate::modules::scene::Context,
        target_entity: crate::modules::scene::EntityId,
    ) -> Vec<crate::modules::scene::EntityId> {
        use crate::modules::scene::*;

        let mut child_entities = Vec::new();
        query_entities(context, crate::modules::scene::PARENT)
            .into_iter()
            .for_each(|entity| {
                if let Some(Parent(parent_entity)) =
                    get_component(context, entity, crate::modules::scene::PARENT)
                {
                    if *parent_entity != target_entity {
                        return;
                    }
                    child_entities.push(entity);
                }
            });
        child_entities
    }

    /// Query for all the descendent entities of a target entity
    pub fn query_descendents(
        context: &crate::modules::scene::Context,
        target_entity: crate::modules::scene::EntityId,
    ) -> Vec<crate::modules::scene::EntityId> {
        let mut descendents = Vec::new();
        let mut stack = vec![target_entity];
        while let Some(entity) = stack.pop() {
            descendents.push(entity);
            query_children(context, entity)
                .into_iter()
                .for_each(|child| {
                    stack.push(child);
                });
        }
        descendents
    }
}
