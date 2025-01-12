use crate::*;

crate::ecs! {
    Context {
        camera: Camera => CAMERA,
        global_transform: GlobalTransform => GLOBAL_TRANSFORM,
        local_transform: LocalTransform => LOCAL_TRANSFORM,
        lines: Lines => LINES,
        quads: Quads => QUADS,
        name: Name => NAME,
        parent: Parent => PARENT,
    }
    Resources {
        window: window::Window,
        graphics: graphics::Graphics,
        input: input::Input,
        user_interface: ui::UserInterface,
        active_camera_entity: Option<EntityId>,
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
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

impl LocalTransform {
    pub fn as_matrix(&self) -> nalgebra_glm::Mat4 {
        nalgebra_glm::translation(&self.translation)
            * nalgebra_glm::quat_to_mat4(&self.rotation.normalize())
            * nalgebra_glm::scaling(&self.scale)
    }

    pub fn right_vector(&self) -> nalgebra_glm::Vec3 {
        extract_right_vector(&self.as_matrix())
    }

    pub fn up_vector(&self) -> nalgebra_glm::Vec3 {
        extract_up_vector(&self.as_matrix())
    }

    pub fn forward_vector(&self) -> nalgebra_glm::Vec3 {
        extract_forward_vector(&self.as_matrix())
    }
}

#[derive(Default, Debug, Copy, Clone, PartialEq)]
pub struct GlobalTransform(pub nalgebra_glm::Mat4);

impl GlobalTransform {
    pub fn right_vector(&self) -> nalgebra_glm::Vec3 {
        extract_right_vector(&self.0)
    }

    pub fn up_vector(&self) -> nalgebra_glm::Vec3 {
        extract_up_vector(&self.0)
    }

    pub fn forward_vector(&self) -> nalgebra_glm::Vec3 {
        extract_forward_vector(&self.0)
    }
}

fn extract_right_vector(transform: &nalgebra_glm::Mat4) -> nalgebra_glm::Vec3 {
    nalgebra_glm::vec3(transform[(0, 0)], transform[(1, 0)], transform[(2, 0)])
}

fn extract_up_vector(transform: &nalgebra_glm::Mat4) -> nalgebra_glm::Vec3 {
    nalgebra_glm::vec3(transform[(0, 1)], transform[(1, 1)], transform[(2, 1)])
}

fn extract_forward_vector(transform: &nalgebra_glm::Mat4) -> nalgebra_glm::Vec3 {
    nalgebra_glm::vec3(-transform[(0, 2)], -transform[(1, 2)], -transform[(2, 2)])
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Name(pub String);

#[derive(Default, Debug, Copy, Clone, PartialEq)]
pub struct Parent(pub crate::context::EntityId);

#[derive(Default, Debug, Copy, Clone, PartialEq)]
pub struct ActiveCamera;

#[derive(Default, Debug, Clone)]
pub struct Lines(pub Vec<Line>);

#[derive(Debug, Clone)]
pub struct Line {
    pub start: nalgebra_glm::Vec3,
    pub end: nalgebra_glm::Vec3,
    pub color: nalgebra_glm::Vec4,
}

#[derive(Default, Debug, Clone)]
pub struct Quads(pub Vec<Quad>);

#[derive(Debug, Clone)]
pub struct Quad {
    pub size: nalgebra_glm::Vec2,
    pub offset: nalgebra_glm::Vec3,
    pub color: nalgebra_glm::Vec4,
}

#[derive(Default, Debug, Clone)]
pub struct Camera {
    pub projection: Projection,
}

impl Camera {
    pub fn projection_matrix(&self, aspect_ratio: f32) -> nalgebra_glm::Mat4 {
        match &self.projection {
            Projection::Perspective(camera) => camera.matrix(aspect_ratio),
            Projection::Orthographic(camera) => camera.matrix(),
        }
    }
}

#[derive(Default, Debug, Copy, Clone)]
pub struct CameraMatrices {
    pub camera_position: nalgebra_glm::Vec3,
    pub projection: nalgebra_glm::Mat4,
    pub view: nalgebra_glm::Mat4,
}

#[derive(Debug, Clone)]
pub enum Projection {
    Perspective(PerspectiveCamera),
    Orthographic(OrthographicCamera),
}

impl Default for Projection {
    fn default() -> Self {
        Self::Perspective(PerspectiveCamera::default())
    }
}

#[derive(Debug, Clone)]
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

#[derive(Default, Debug, Clone)]
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

/// Queries for root nodes by looking for entities that do not have a Parent component
pub fn query_root_nodes(context: &Context) -> Vec<EntityId> {
    let mut root_entities: Vec<EntityId> = context
        .tables
        .iter()
        .filter_map(|table| {
            if crate::has_components!(table, PARENT) {
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
pub fn query_children(context: &Context, target_entity: EntityId) -> Vec<EntityId> {
    let mut child_entities = Vec::new();
    query_entities(context, PARENT)
        .into_iter()
        .for_each(|entity| {
            let Some(Parent(parent_entity)) = get_component(context, entity, PARENT) else {
                return;
            };
            if *parent_entity != target_entity {
                return;
            }
            child_entities.push(entity);
        });
    child_entities
}

/// Query for all the descendent entities of a target entity
pub fn query_descendents(context: &Context, target_entity: EntityId) -> Vec<EntityId> {
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

pub fn query_global_transform(context: &Context, entity: EntityId) -> nalgebra_glm::Mat4 {
    let Some(local_transform) = get_component::<LocalTransform>(context, entity, LOCAL_TRANSFORM)
    else {
        return nalgebra_glm::Mat4::identity();
    };
    if let Some(Parent(parent)) = get_component::<Parent>(context, entity, PARENT) {
        query_global_transform(context, *parent) * local_transform.as_matrix()
    } else {
        local_transform.as_matrix()
    }
}

pub fn query_active_camera_matrices(context: &Context) -> Option<CameraMatrices> {
    let active_camera = context.resources.active_camera_entity?;
    query_camera_matrices(context, active_camera)
}

pub fn query_camera_matrices(context: &Context, camera_entity: EntityId) -> Option<CameraMatrices> {
    let (Some(camera), Some(local_transform), Some(global_transform)) = (
        get_component::<Camera>(context, camera_entity, CAMERA),
        get_component::<LocalTransform>(context, camera_entity, LOCAL_TRANSFORM),
        get_component::<GlobalTransform>(context, camera_entity, GLOBAL_TRANSFORM),
    ) else {
        return None;
    };

    let normalized_rotation = local_transform.rotation.normalize();
    let camera_translation = global_transform.0.column(3).xyz();
    let target = camera_translation
        + nalgebra_glm::quat_rotate_vec3(&normalized_rotation, &(-nalgebra_glm::Vec3::z()));
    let up = nalgebra_glm::quat_rotate_vec3(&normalized_rotation, &nalgebra_glm::Vec3::y());

    let aspect_ratio = window::query_viewport_aspect_ratio(context).unwrap_or(4.0 / 3.0);

    Some(CameraMatrices {
        camera_position: camera_translation,
        projection: camera.projection_matrix(aspect_ratio),
        view: nalgebra_glm::look_at(&camera_translation, &target, &up),
    })
}

/// Pure query function - only returns the nth camera entity
pub fn query_nth_camera(context: &Context, index: usize) -> Option<EntityId> {
    query_entities(context, CAMERA).get(index).copied()
}

/// Initializes a camera with proper transform settings
pub fn initialize_camera_transform(context: &mut Context, camera_entity: EntityId) {
    if let Some(local_transform) = 
        get_component_mut::<LocalTransform>(context, camera_entity, LOCAL_TRANSFORM) 
    {
        // Set a default position offset from origin
        local_transform.translation = nalgebra_glm::vec3(0.0, 4.0, 5.0);
        
        // Ensure rotation is looking at origin with proper up vector
        let camera_pos = local_transform.translation;
        let target = nalgebra_glm::Vec3::zeros();
        let up = nalgebra_glm::Vec3::y();
        
        // Calculate rotation to look at target
        let forward = nalgebra_glm::normalize(&(target - camera_pos));
        let right = nalgebra_glm::normalize(&nalgebra_glm::cross(&up, &forward));
        let new_up = nalgebra_glm::cross(&forward, &right);
        
        // Convert to quaternion
        let rotation_mat = nalgebra_glm::mat3(
            right.x, new_up.x, -forward.x,
            right.y, new_up.y, -forward.y, 
            right.z, new_up.z, -forward.z
        );
        local_transform.rotation = nalgebra_glm::mat3_to_quat(&rotation_mat);
    }
}

/// System that ensures all cameras have proper initialization
pub fn ensure_cameras_initialized_system(context: &mut Context) {
    let camera_entities: Vec<_> = query_entities(context, CAMERA)
        .into_iter()
        .filter(|entity| !get_component::<LocalTransform>(context, *entity, LOCAL_TRANSFORM).is_some())
        .collect();
        
    for entity in camera_entities {
        add_components(context, entity, LOCAL_TRANSFORM);
        initialize_camera_transform(context, entity);
    }
}

pub fn query_nth_camera_matrices(
    context: &mut crate::Context,
    index: usize,
) -> Option<crate::prelude::CameraMatrices> {
    use crate::context::*;
    let camera_entity = query_nth_camera(context, index)?;
    let matrices = query_camera_matrices(context, camera_entity)?;
    Some(matrices)
}

pub fn ensure_main_camera_system(context: &mut Context) {
    if context.resources.active_camera_entity.is_some() {
        return;
    }
    let camera_entity = if let Some(first_camera) = query_entities(context, CAMERA).first() {
        *first_camera
    } else {
        let camera_mask = CAMERA | LOCAL_TRANSFORM | GLOBAL_TRANSFORM | NAME;
        spawn_entities(context, camera_mask, 1)[0]
    };

    if let Some(name) = get_component_mut::<Name>(context, camera_entity, NAME) {
        *name = Name("Main Camera".to_string());
    }
    if let Some(local_transform) =
        get_component_mut::<LocalTransform>(context, camera_entity, LOCAL_TRANSFORM)
    {
        local_transform.translation = nalgebra_glm::vec3(0.0, 4.0, 5.0);
    }
}

pub fn wasd_keyboard_controls_system(context: &mut Context) {
    let Some(camera_entity) = context.resources.active_camera_entity else {
        return;
    };
    let delta_time = context.resources.window.delta_time;
    let speed = 10.0 * delta_time;

    let (
        left_key_pressed,
        right_key_pressed,
        forward_key_pressed,
        backward_key_pressed,
        up_key_pressed,
    ) = {
        let keyboard = &context.resources.input.keyboard;
        (
            keyboard.is_key_pressed(winit::keyboard::KeyCode::KeyA),
            keyboard.is_key_pressed(winit::keyboard::KeyCode::KeyD),
            keyboard.is_key_pressed(winit::keyboard::KeyCode::KeyW),
            keyboard.is_key_pressed(winit::keyboard::KeyCode::KeyS),
            keyboard.is_key_pressed(winit::keyboard::KeyCode::Space),
        )
    };

    let Some(local_transform) =
        get_component_mut::<LocalTransform>(context, camera_entity, LOCAL_TRANSFORM)
    else {
        return;
    };
    let local_transform_matrix = local_transform.as_matrix();
    let forward = extract_forward_vector(&local_transform_matrix);
    let right = extract_right_vector(&local_transform_matrix);
    let up = extract_up_vector(&local_transform_matrix);

    if forward_key_pressed {
        local_transform.translation += forward * speed;
    }
    if backward_key_pressed {
        local_transform.translation -= forward * speed;
    }

    if left_key_pressed {
        local_transform.translation -= right * speed;
    }
    if right_key_pressed {
        local_transform.translation += right * speed;
    }
    if up_key_pressed {
        local_transform.translation += up * speed;
    }
}

/// Updates the active camera's orientation using
/// mouse controls for orbiting and panning
pub fn look_camera_system(context: &mut Context) {
    let Some(camera_entity) = context.resources.active_camera_entity else {
        return;
    };
    let (local_transform_matrix, _, right, up) = {
        let Some(local_transform) =
            get_component_mut::<LocalTransform>(context, camera_entity, LOCAL_TRANSFORM)
        else {
            return;
        };
        let local_transform_matrix = local_transform.as_matrix();

        let forward = extract_forward_vector(&local_transform_matrix);
        let right = extract_right_vector(&local_transform_matrix);
        let up = extract_up_vector(&local_transform_matrix);
        (local_transform_matrix, forward, right, up)
    };

    if context
        .resources
        .input
        .mouse
        .state
        .contains(input::MouseState::RIGHT_CLICKED)
    {
        let mut delta =
            context.resources.input.mouse.position_delta * context.resources.window.delta_time;
        delta.x *= -1.0;
        delta.y *= -1.0;

        let Some(local_transform) =
            get_component_mut::<LocalTransform>(context, camera_entity, LOCAL_TRANSFORM)
        else {
            return;
        };

        let yaw = nalgebra_glm::quat_angle_axis(delta.x, &nalgebra_glm::Vec3::y());
        local_transform.rotation = yaw * local_transform.rotation;

        let forward = extract_forward_vector(&local_transform_matrix);
        let current_pitch = forward.y.asin();

        let new_pitch = current_pitch + delta.y;
        if new_pitch.abs() <= 89_f32.to_radians() {
            let pitch = nalgebra_glm::quat_angle_axis(delta.y, &nalgebra_glm::Vec3::x());
            local_transform.rotation *= pitch;
        }
    }

    if context
        .resources
        .input
        .mouse
        .state
        .contains(input::MouseState::MIDDLE_CLICKED)
    {
        let mut delta =
            context.resources.input.mouse.position_delta * context.resources.window.delta_time;
        delta.x *= -1.0;
        delta.y *= -1.0;

        let Some(local_transform) =
            get_component_mut::<LocalTransform>(context, camera_entity, LOCAL_TRANSFORM)
        else {
            return;
        };
        local_transform.translation += right * delta.x;
        local_transform.translation += up * delta.y;
    }
}

/// Uses the `Parent` component and right-multiplied
/// local transform mat4's to calculate the global transform of each entity
pub fn update_global_transforms_system(context: &mut Context) {
    query_entities(context, LOCAL_TRANSFORM | GLOBAL_TRANSFORM)
        .into_iter()
        .for_each(|entity| {
            let new_global_transform = query_global_transform(context, entity);
            let global_transform =
                get_component_mut::<GlobalTransform>(context, entity, GLOBAL_TRANSFORM).unwrap();
            *global_transform = GlobalTransform(new_global_transform);
        });
}
