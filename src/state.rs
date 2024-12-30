crate::ecs! {
    Context {
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
    pub struct Parent(pub crate::EntityId);

    #[derive(Default, Debug, Copy, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
    pub struct Visible;
}

pub use resources::*;
pub mod resources {
    pub use window::*;
    pub mod window {
        /// Contains resources for window creation and destruction
        #[derive(Default)]
        pub struct Window {
            pub handle: Option<std::sync::Arc<winit::window::Window>>,
            pub should_exit: bool,
        }

        /// Contains timing information about the current frame
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
    }

    pub use renderer::*;
    pub mod renderer {
        /// A resource for graphics state
        #[derive(Default)]
        pub struct Graphics {
            /// The renderer context
            pub renderer: Option<Renderer>,

            /// The renderer context
            #[cfg(target_arch = "wasm32")]
            pub renderer_receiver: Option<futures::channel::oneshot::Receiver<crate::Renderer>>,

            /// The size of the display viewport
            pub viewport_size: (u32, u32),
        }

        /// Contains all resources required for rendering
        pub struct Renderer {
            pub gpu: Gpu,
            pub depth_texture_view: wgpu::TextureView,
            pub egui_renderer: egui_wgpu::Renderer,
            pub depth_format: wgpu::TextureFormat,
            pub triangle: TriangleRender,
        }

        /// Low-level wgpu handles
        pub struct Gpu {
            pub surface: wgpu::Surface<'static>,
            pub device: wgpu::Device,
            pub queue: wgpu::Queue,
            pub surface_config: wgpu::SurfaceConfiguration,
            pub surface_format: wgpu::TextureFormat,
        }

        /// Common vertex format for all triangle mesh rendering
        #[repr(C)]
        #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
        pub struct Vertex {
            pub position: [f32; 4],
            pub color: [f32; 4],
        }

        use triangle::*;
        pub mod triangle {
            pub struct TriangleRender {
                pub model: nalgebra_glm::Mat4,
                pub vertex_buffer: wgpu::Buffer,
                pub index_buffer: wgpu::Buffer,
                pub buffer: wgpu::Buffer,
                pub bind_group: wgpu::BindGroup,
                pub bind_group_layout: wgpu::BindGroupLayout,
                pub pipeline: wgpu::RenderPipeline,
            }

            #[repr(C)]
            #[derive(Default, Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
            pub struct UniformBuffer {
                pub mvp: nalgebra_glm::Mat4,
            }

            pub const TRIANGLE_VERTICES: [crate::Vertex; 3] = [
                crate::Vertex {
                    position: [1.0, -1.0, 0.0, 1.0],
                    color: [1.0, 0.0, 0.0, 1.0],
                },
                crate::Vertex {
                    position: [-1.0, -1.0, 0.0, 1.0],
                    color: [0.0, 1.0, 0.0, 1.0],
                },
                crate::Vertex {
                    position: [0.0, 1.0, 0.0, 1.0],
                    color: [0.0, 0.0, 1.0, 1.0],
                },
            ];

            pub const TRIANGLE_INDICES: [u32; 3] = [0, 1, 2]; // Clockwise winding order
        }
    }

    pub use user_interface::*;
    pub mod user_interface {
        #[derive(Default)]
        pub struct UserInterface {
            pub state: Option<egui_winit::State>,
            pub tile_tree: Option<egui_tiles::Tree<Pane>>,
            pub tile_tree_behavior: TileTreeContext,
            pub frame_output: Option<(egui::FullOutput, Vec<egui::ClippedPrimitive>)>,
            pub show_left_panel: bool,
            pub show_right_panel: bool,
            pub consumed_event: bool,
            pub selected_entity: Option<crate::EntityId>,
        }

        /// Panes display in resizable tiles in the application
        #[derive(Default, Debug, Copy, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
        pub struct Pane {}

        /// A context shared between all the panes in the tile tree
        #[derive(Default)]
        pub struct TileTreeContext {
            pub tile_rects: std::collections::HashMap<egui_tiles::TileId, egui::Rect>,
            pub add_child_to: Option<egui_tiles::TileId>,
        }
    }

    pub use input::*;
    pub mod input {
        /// Contains all input state
        #[derive(Default)]
        pub struct Input {
            pub keyboard: Keyboard,
            pub mouse: Mouse,
        }

        /// Contains keyboard-specific input state
        #[derive(Default)]
        pub struct Keyboard {
            pub keystates:
                std::collections::HashMap<winit::keyboard::KeyCode, winit::event::ElementState>,
        }

        bitflags::bitflags! {
            #[derive(Default, Debug, Clone, Copy)]
            pub struct MouseState: u8 {
                const LEFT_CLICKED = 0b0000_0001;
                const MIDDLE_CLICKED = 0b0000_0010;
                const RIGHT_CLICKED = 0b0000_0100;
                const MOVED = 0b0000_1000;
                const SCROLLED = 0b0001_0000;
            }
        }

        /// Contains mouse-specific input state
        #[derive(Default, Debug, Clone, Copy)]
        pub struct Mouse {
            pub state: MouseState,
            pub position: nalgebra_glm::Vec2,
            pub position_delta: nalgebra_glm::Vec2,
            pub wheel_delta: nalgebra_glm::Vec2,
        }
    }
}
