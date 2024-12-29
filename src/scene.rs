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
    pub struct Graphics {
        pub renderer: Option<crate::graphics::Renderer>,
        #[cfg(target_arch = "wasm32")]
        pub renderer_receiver:
            Option<futures::channel::oneshot::Receiver<crate::graphics::Renderer>>,
        pub viewport_size: (u32, u32),
    }

    #[derive(Default)]
    pub struct UserInterface {
        pub state: Option<egui_winit::State>,
        pub tile_tree: Option<egui_tiles::Tree<Pane>>,
        pub tile_tree_behavior: TreeBehavior,
        pub frame_output: Option<(egui::FullOutput, Vec<egui::ClippedPrimitive>)>,
        pub show_left_panel: bool,
        pub show_right_panel: bool,
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
