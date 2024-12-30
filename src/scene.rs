crate::ecs! {
    Scene {
        local_transform: LocalTransform => LOCAL_TRANSFORM,
        global_transform: GlobalTransform => GLOBAL_TRANSFORM,
        name: GlobalTransform => NAME,
        parent: Parent => PARENT,
        visible: Visible => VISIBLE,
    }
    Resources {
        #[serde(skip)] window: Window,
        #[serde(skip)] graphics: Graphics,
        #[serde(skip)] input: Input,
        #[serde(skip)] frame_timing: FrameTiming,
        #[serde(skip)] user_interface: UserInterface,
    }
}

pub use components::*;
pub mod components {
    use super::*;

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

    impl From<LocalTransform> for nalgebra_glm::Mat4 {
        fn from(local_transform: LocalTransform) -> Self {
            let LocalTransform {
                translation,
                rotation,
                scale,
            } = local_transform;
            nalgebra_glm::translation(&translation)
                * nalgebra_glm::quat_to_mat4(&rotation)
                * nalgebra_glm::scaling(&scale)
        }
    }

    #[derive(Default, Debug, Copy, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
    pub struct GlobalTransform(pub nalgebra_glm::Mat4);

    #[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
    pub struct Name(pub String);

    #[derive(Default, Debug, Copy, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
    pub struct Parent(pub EntityId);

    #[derive(Default, Debug, Copy, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
    pub struct Visible;
}

pub use resources::*;
pub mod resources {
    use std::collections::{HashMap, HashSet};

    #[derive(Default)]
    pub struct Window {
        pub handle: Option<std::sync::Arc<winit::window::Window>>,
        pub should_exit: bool,
    }

    #[derive(Default)]
    pub struct Input {
        pub keyboard: Keyboard,
        pub mouse: Mouse,
    }

    pub use renderer::*;
    pub mod renderer {
        #[derive(Default)]
        pub struct Graphics {
            pub renderer: Option<Renderer>,
            #[cfg(target_arch = "wasm32")]
            pub renderer_receiver: Option<futures::channel::oneshot::Receiver<crate::Renderer>>,
            pub viewport_size: (u32, u32),
        }

        pub struct Renderer {
            pub gpu: Gpu,
            pub depth_texture_view: wgpu::TextureView,
            pub egui_renderer: egui_wgpu::Renderer,
            pub depth_format: wgpu::TextureFormat,
            pub triangle: TriangleRender,
        }

        pub struct Gpu {
            pub surface: wgpu::Surface<'static>,
            pub device: wgpu::Device,
            pub queue: wgpu::Queue,
            pub surface_config: wgpu::SurfaceConfiguration,
            pub surface_format: wgpu::TextureFormat,
        }

        pub struct TriangleRender {
            pub model: nalgebra_glm::Mat4,
            pub vertex_buffer: wgpu::Buffer,
            pub index_buffer: wgpu::Buffer,
            pub uniform: UniformBinding,
            pub pipeline: wgpu::RenderPipeline,
        }

        pub struct UniformBinding {
            pub buffer: wgpu::Buffer,
            pub bind_group: wgpu::BindGroup,
            pub bind_group_layout: wgpu::BindGroupLayout,
        }

        #[repr(C)]
        #[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        pub struct UniformBuffer {
            pub mvp: nalgebra_glm::Mat4,
        }

        #[repr(C)]
        #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
        pub struct Vertex {
            pub position: [f32; 4],
            pub color: [f32; 4],
        }

        impl Vertex {
            pub fn vertex_attributes() -> Vec<wgpu::VertexAttribute> {
                wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4].to_vec()
            }

            pub fn description(attributes: &[wgpu::VertexAttribute]) -> wgpu::VertexBufferLayout {
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes,
                }
            }
        }

        pub const VERTICES: [crate::Vertex; 3] = [
            Vertex {
                position: [1.0, -1.0, 0.0, 1.0],
                color: [1.0, 0.0, 0.0, 1.0],
            },
            Vertex {
                position: [-1.0, -1.0, 0.0, 1.0],
                color: [0.0, 1.0, 0.0, 1.0],
            },
            Vertex {
                position: [0.0, 1.0, 0.0, 1.0],
                color: [0.0, 0.0, 1.0, 1.0],
            },
        ];

        pub const INDICES: [u32; 3] = [0, 1, 2]; // Clockwise winding order
    }

    #[derive(Default)]
    pub struct UserInterface {
        pub state: Option<egui_winit::State>,
        pub tile_tree: Option<egui_tiles::Tree<Pane>>,
        pub tile_tree_behavior: TreeBehavior,
        pub frame_output: Option<(egui::FullOutput, Vec<egui::ClippedPrimitive>)>,
        pub show_left_panel: bool,
        pub show_right_panel: bool,
        pub consumed_event: bool,
        pub selected_entity: Option<crate::EntityId>,
    }

    #[derive(Default, Debug, Copy, Clone, PartialEq)]
    pub struct FrameTiming {
        /// The number of frames rendered per second
        pub frames_per_second: f32,

        /// The time between the last frame and the current frame
        pub delta_time: f32,

        /// The time the current frame was started
        pub last_frame_start_instant: Option<web_time::Instant>,

        /// When the current frame started
        pub current_frame_start_instant: Option<web_time::Instant>,

        /// When the initial frame started, when the application starts up
        pub initial_frame_start_instant: Option<web_time::Instant>,

        /// A monotonically increasing counter incremented each frame
        pub frame_counter: u32,

        /// Milliseconds that the process has been running continuously
        pub uptime_milliseconds: u64,
    }

    #[derive(Default)]
    pub struct Keyboard {
        pub keystates:
            std::collections::HashMap<winit::keyboard::KeyCode, winit::event::ElementState>,
    }

    impl Keyboard {
        pub fn is_key_pressed(&self, keycode: winit::keyboard::KeyCode) -> bool {
            self.keystates.contains_key(&keycode)
                && self.keystates[&keycode] == winit::event::ElementState::Pressed
        }
    }

    bitflags::bitflags! {
        #[derive(Default, Debug, Clone, Copy)]
        pub struct MouseButtons: u8 {
            const LEFT_CLICKED = 0b0000_0001;
            const MIDDLE_CLICKED = 0b0000_0010;
            const RIGHT_CLICKED = 0b0000_0100;
            const MOVED = 0b0000_1000;
            const SCROLLED = 0b0001_0000;
        }
    }

    #[derive(Default, Debug, Clone, Copy)]
    pub struct Mouse {
        pub buttons: MouseButtons,
        pub position: nalgebra_glm::Vec2,
        pub position_delta: nalgebra_glm::Vec2,
        pub wheel_delta: nalgebra_glm::Vec2,
    }

    #[derive(Default, Debug, Copy, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
    pub struct Pane {}

    #[derive(Default)]
    pub struct TreeBehavior {
        pub visible_tiles: HashSet<egui_tiles::TileId>,
        pub tile_rects: HashMap<egui_tiles::TileId, egui::Rect>,
        pub add_child_to: Option<egui_tiles::TileId>,
    }

    impl egui_tiles::Behavior<Pane> for TreeBehavior {
        fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
            24.0
        }

        fn gap_width(&self, _style: &egui::Style) -> f32 {
            2.0
        }

        fn is_tab_closable(
            &self,
            _tiles: &egui_tiles::Tiles<Pane>,
            _tile_id: egui_tiles::TileId,
        ) -> bool {
            true
        }

        fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
            egui_tiles::SimplificationOptions {
                all_panes_must_have_tabs: true,
                ..Default::default()
            }
        }

        fn tab_title_for_pane(&mut self, _pane: &Pane) -> egui::WidgetText {
            "Pane".into()
        }

        fn top_bar_right_ui(
            &mut self,
            _tiles: &egui_tiles::Tiles<Pane>,
            ui: &mut egui::Ui,
            tile_id: egui_tiles::TileId,
            _tabs: &egui_tiles::Tabs,
            _scroll_offset: &mut f32,
        ) {
            if ui.button("➕").clicked() {
                self.add_child_to = Some(tile_id);
            }
        }

        fn pane_ui(
            &mut self,
            ui: &mut egui::Ui,
            tile_id: egui_tiles::TileId,
            _pane: &mut Pane,
        ) -> egui_tiles::UiResponse {
            let rect = ui.max_rect();

            // Store this tile's rect for overlap checking
            self.tile_rects.insert(tile_id, rect);

            // Display tile ID in the center of each pane
            ui.centered_and_justified(|ui| {
                ui.label(format!("Tile {}", tile_id.0));
            });

            if ui.button("Click Me").clicked() {
                println!("Button clicked in Pane");
            }

            // Only enable dragging when shift is pressed
            let shift_pressed = ui.input(|i| i.modifiers.shift);
            let cursor = if shift_pressed {
                egui::CursorIcon::Grab
            } else {
                egui::CursorIcon::Default
            };

            let response = ui
                .allocate_rect(ui.max_rect(), egui::Sense::click_and_drag())
                .on_hover_cursor(cursor);

            if shift_pressed && response.dragged() {
                egui_tiles::UiResponse::DragStarted
            } else {
                egui_tiles::UiResponse::None
            }
        }
    }
}
