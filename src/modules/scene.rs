crate::ecs! {
    Context {
        camera: Camera => CAMERA,
        global_transform: GlobalTransform => GLOBAL_TRANSFORM,
        local_transform: LocalTransform => LOCAL_TRANSFORM,
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

pub use components::*;
pub mod components {
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

    #[derive(Default, Debug, serde::Serialize, serde::Deserialize, Clone)]
    pub struct Camera {
        pub projection: Projection,
        pub viewport: Option<Viewport>,
        pub tile_id: Option<egui_tiles::TileId>,
    }

    impl Camera {
        pub fn projection_matrix(&self, aspect_ratio: f32) -> nalgebra_glm::Mat4 {
            match &self.projection {
                Projection::Perspective(camera) => camera.matrix(aspect_ratio),
                Projection::Orthographic(camera) => camera.matrix(),
            }
        }
    }

    #[derive(Default, Debug, Copy, Clone, serde::Serialize, serde::Deserialize)]
    pub struct Viewport {
        pub x: u32,
        pub y: u32,
        pub width: u32,
        pub height: u32,
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
    pub enum Projection {
        Perspective(PerspectiveCamera),
        Orthographic(OrthographicCamera),
    }

    impl Default for Projection {
        fn default() -> Self {
            Self::Perspective(PerspectiveCamera::default())
        }
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
    pub struct PerspectiveCamera {
        pub aspect_ratio: Option<f32>,
        pub y_fov_rad: f32,
        pub z_far: Option<f32>,
        pub z_near: f32,
    }

    impl Default for PerspectiveCamera {
        fn default() -> Self {
            Self {
                aspect_ratio: None,
                y_fov_rad: 90_f32.to_radians(),
                z_far: None,
                z_near: 0.01,
            }
        }
    }

    impl PerspectiveCamera {
        pub fn matrix(&self, viewport_aspect_ratio: f32) -> nalgebra_glm::Mat4 {
            let aspect_ratio = if let Some(aspect_ratio) = self.aspect_ratio {
                aspect_ratio
            } else {
                viewport_aspect_ratio
            };

            if let Some(z_far) = self.z_far {
                nalgebra_glm::perspective_zo(aspect_ratio, self.y_fov_rad, self.z_near, z_far)
            } else {
                nalgebra_glm::infinite_perspective_rh_zo(aspect_ratio, self.y_fov_rad, self.z_near)
            }
        }
    }

    #[derive(Default, Debug, serde::Serialize, serde::Deserialize, Clone)]
    pub struct OrthographicCamera {
        pub x_mag: f32,
        pub y_mag: f32,
        pub z_far: f32,
        pub z_near: f32,
    }

    impl OrthographicCamera {
        pub fn matrix(&self) -> nalgebra_glm::Mat4 {
            let z_sum = self.z_near + self.z_far;
            let z_diff = self.z_near - self.z_far;
            nalgebra_glm::Mat4::new(
                1.0 / self.x_mag,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0 / self.y_mag,
                0.0,
                0.0,
                0.0,
                0.0,
                2.0 / z_diff,
                0.0,
                0.0,
                0.0,
                z_sum / z_diff,
                1.0,
            )
        }
    }
}

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
