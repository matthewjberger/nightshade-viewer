use crate::{
    api::{push_command, Command, EntityCommand, RequestCommand},
    network::{NetworkCommand, NetworkMessage},
    paint::{paint_cube_scene, paint_entity},
};

#[derive(Default)]
pub struct UserInterface {
    pub state: Option<egui_winit::State>,
    pub tile_tree: Option<egui_tiles::Tree<Pane>>,
    pub tile_tree_context: TileTreeContext,
    pub frame_output: Option<(egui::FullOutput, Vec<egui::ClippedPrimitive>)>,
    pub show_left_panel: bool,
    pub show_right_panel: bool,
    pub show_bottom_panel: bool,
    pub uniform_scaling: bool,
    pub consumed_event: bool,
    pub selected_entity: Option<crate::context::EntityId>,
    pub timeline_state: TimelineState,
    pub backend_websocket_address: String,
}

/// A context shared between all the panes in the tile tree
#[derive(Default)]
pub struct TileTreeContext {
    pub tile_rects: std::collections::HashMap<egui_tiles::TileId, egui::Rect>,
    pub add_child_to: Option<egui_tiles::TileId>,
    pub viewport_tiles: std::collections::HashMap<egui_tiles::TileId, (PaneKind, egui::Rect)>,
    pub selected_tile: Option<egui_tiles::TileId>,
    pub tile_mapping: std::collections::HashMap<egui_tiles::TileId, usize>,
    pub context: Option<*mut crate::context::Context>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PaneKind {
    Scene {
        scene_index: usize,
        active_camera_index: usize,
    },
    Color(egui::Color32),
}

impl Default for PaneKind {
    fn default() -> Self {
        Self::Scene {
            scene_index: 0,
            active_camera_index: 0,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Pane {
    pub kind: PaneKind,
}

impl egui_tiles::Behavior<crate::ui::Pane> for crate::ui::TileTreeContext {
    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        24.0
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        2.0
    }

    fn is_tab_closable(
        &self,
        _tiles: &egui_tiles::Tiles<crate::ui::Pane>,
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

    fn tab_title_for_pane(&mut self, pane: &crate::ui::Pane) -> egui::WidgetText {
        match pane.kind {
            PaneKind::Scene {
                scene_index: _,
                active_camera_index: _,
            } => "Scene".into(),
            PaneKind::Color(_) => "Color".into(),
        }
    }

    fn top_bar_right_ui(
        &mut self,
        _tiles: &egui_tiles::Tiles<crate::ui::Pane>,
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
        pane: &mut crate::ui::Pane,
    ) -> egui_tiles::UiResponse {
        let Some(Some(context)) = self.context.as_mut().map(|ctx| unsafe { ctx.as_mut() }) else {
            return egui_tiles::UiResponse::None;
        };

        let rect = ui.max_rect();
        self.tile_rects.insert(tile_id, rect);

        if matches!(pane.kind, PaneKind::Scene { .. } | PaneKind::Color(_)) {
            self.viewport_tiles.insert(tile_id, (pane.kind, rect));
        }

        // Draw selection border only for selected tile
        if self.selected_tile == Some(tile_id) {
            let border_color = egui::Color32::from_rgb(251, 146, 60);
            let border_width = 8.0;
            let border_rounding = 6.0;

            let border_rect = rect.shrink(1.0);
            ui.painter().rect_stroke(
                border_rect,
                border_rounding,
                egui::Stroke::new(border_width, border_color),
            );
        }

        match pane.kind {
            PaneKind::Scene {
                active_camera_index: _,
                scene_index: _,
            } => {
                // Empty - we'll render the camera view elsewhere
            }
            PaneKind::Color(_) => {
                // TODO: demonstrate a regular egui widget here
            }
        }

        let mut drag_response = egui_tiles::UiResponse::None;

        let controls_height = 28.0;
        let (controls_rect, content_rect) =
            rect.split_top_bottom_at_y(rect.min.y + controls_height);

        let controls_response = ui.allocate_ui_with_layout(
            controls_rect.size(),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.add_space(4.0);

                egui::ComboBox::new(format!("background_{}", tile_id.0), "")
                    .selected_text(match pane.kind {
                        PaneKind::Scene { .. } => "Scene",
                        PaneKind::Color(_) => "Color",
                    })
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        // Show Scene option
                        let is_scene = matches!(pane.kind, PaneKind::Scene { .. });
                        if ui.selectable_label(is_scene, "Scene").clicked() {
                            pane.kind = PaneKind::Scene {
                                scene_index: 0,
                                active_camera_index: 0,
                            };
                        }

                        // Show Color option
                        let is_color = matches!(pane.kind, PaneKind::Color(_));
                        if ui.selectable_label(is_color, "Color").clicked() {
                            pane.kind = PaneKind::Color(egui::Color32::from_gray(200));
                        }
                    });

                if let PaneKind::Scene {
                    active_camera_index,
                    scene_index: _,
                } = &mut pane.kind
                {
                    ui.add_space(4.0);

                    let camera_count =
                        crate::context::query_entities(context, crate::context::CAMERA).len();
                    egui::ComboBox::new(format!("camera_{}", tile_id.0), "")
                        .selected_text(format!("Camera {}", *active_camera_index + 1))
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            for i in 0..camera_count {
                                if ui
                                    .selectable_label(
                                        *active_camera_index == i,
                                        format!("Camera {}", i + 1),
                                    )
                                    .clicked()
                                {
                                    *active_camera_index = i;
                                }
                            }
                        });
                }

                if let PaneKind::Color(ref mut color) = pane.kind {
                    ui.add_space(4.0);
                    ui.color_edit_button_srgba(color);
                }
            },
        );

        let shift_pressed = ui.input(|i| i.modifiers.shift);
        if shift_pressed && !controls_response.response.hovered() {
            let cursor = egui::CursorIcon::Move;
            let response = ui
                .allocate_rect(content_rect, egui::Sense::click_and_drag())
                .on_hover_cursor(cursor);

            if response.dragged() {
                drag_response = egui_tiles::UiResponse::DragStarted;
            }
        }

        drag_response
    }
}

pub fn receive_ui_event(context: &mut crate::context::Context, event: &winit::event::WindowEvent) {
    let Some(gui_state) = &mut context.resources.user_interface.state else {
        return;
    };
    let Some(window_handle) = context.resources.window.handle.as_ref() else {
        return;
    };
    context.resources.user_interface.consumed_event =
        gui_state.on_window_event(window_handle, event).consumed;

    let mut new_selected_tile = None;

    if let winit::event::WindowEvent::MouseInput {
        state: winit::event::ElementState::Pressed,
        button: winit::event::MouseButton::Left,
        ..
    } = event
    {
        let mouse_pos = context.resources.input.mouse.position;
        let mouse_pos = egui::pos2(mouse_pos.x, mouse_pos.y);

        context
            .resources
            .user_interface
            .tile_tree_context
            .selected_tile = None;
        for (tile_id, rect) in &context
            .resources
            .user_interface
            .tile_tree_context
            .tile_rects
        {
            if rect.contains(mouse_pos) {
                context
                    .resources
                    .user_interface
                    .tile_tree_context
                    .selected_tile = Some(*tile_id);
                new_selected_tile = Some(*tile_id);
                break;
            }
        }
    }

    if let Some(selected_tile) = new_selected_tile {
        if let Some((pane_kind, _rect)) = context
            .resources
            .user_interface
            .tile_tree_context
            .viewport_tiles
            .get(&selected_tile)
        {
            match pane_kind {
                PaneKind::Scene {
                    active_camera_index,
                    scene_index: _,
                } => {
                    // Get total number of cameras
                    let camera_count =
                        crate::context::query_entities(context, crate::context::CAMERA).len();

                    // Only try to select camera if index is valid
                    if *active_camera_index < camera_count {
                        if let Some(camera_entity) =
                            crate::context::query_nth_camera(context, *active_camera_index)
                        {
                            context.resources.active_camera_entity = Some(camera_entity);
                        }
                    }
                }
                PaneKind::Color(_color32) => {
                    //
                }
            }
        }
    }
}

/// Resizes the egui UI, ensuring it matches the window scale factor
pub fn resize_ui(context: &mut crate::context::Context) {
    let (Some(window_handle), Some(gui_state)) = (
        context.resources.window.handle.as_ref(),
        context.resources.user_interface.state.as_mut(),
    ) else {
        return;
    };
    gui_state
        .egui_ctx()
        .set_pixels_per_point(window_handle.scale_factor() as f32);
}

/// Ensures a default layout when the tile tree is emptied
pub fn ensure_tile_tree_system(context: &mut crate::context::Context) {
    if let Some(tile_tree) = &context.resources.user_interface.tile_tree {
        if !tile_tree.tiles.is_empty() {
            return;
        }
    }
    let mut tiles = egui_tiles::Tiles::default();
    let mut tab_tiles = vec![];
    let tab_tile_child = tiles.insert_pane(crate::ui::Pane::default());
    let tab_tile = tiles.insert_tab_tile(vec![tab_tile_child]);
    tab_tiles.push(tab_tile);
    let root = tiles.insert_tab_tile(tab_tiles);
    let tiles = egui_tiles::Tree::new("tree", root, tiles);
    context.resources.user_interface.tile_tree = Some(tiles);
}

/// Creates the UI for the frame and
/// emits the resources needed for rendering
pub fn create_ui_system(context: &mut crate::context::Context) {
    update_timeline_system(context);

    // Set the context pointer before any UI work
    context.resources.user_interface.tile_tree_context.context = Some(context as *mut _);

    let ui = {
        let Some(gui_state) = context.resources.user_interface.state.as_mut() else {
            return;
        };
        let Some(window_handle) = context.resources.window.handle.as_ref() else {
            return;
        };
        let gui_input = gui_state.take_egui_input(window_handle);
        gui_state.egui_ctx().begin_pass(gui_input);
        gui_state.egui_ctx().clone()
    };

    create_ui(context, &ui);

    let Some(gui_state) = context.resources.user_interface.state.as_mut() else {
        return;
    };
    let Some(window_handle) = context.resources.window.handle.as_ref() else {
        return;
    };
    let output = ui.end_pass();
    gui_state.handle_platform_output(window_handle, output.platform_output.clone());
    let paint_jobs = ui.tessellate(output.shapes.clone(), output.pixels_per_point);
    context.resources.user_interface.frame_output = Some((output, paint_jobs));
}

fn create_ui(context: &mut crate::context::Context, ui: &egui::Context) {
    top_panel_ui(context, ui);
    left_panel_ui(context, ui);
    right_panel_ui(context, ui);
    central_panel_ui(context, ui);
    bottom_panel_ui(context, ui);
}

fn central_panel_ui(context: &mut crate::context::Context, ui: &egui::Context) {
    egui::CentralPanel::default()
        .frame(egui::Frame::none())
        .show(ui, |ui| {
            context
                .resources
                .user_interface
                .tile_tree_context
                .viewport_tiles
                .clear();
            context
                .resources
                .user_interface
                .tile_tree_context
                .tile_rects
                .clear();

            let crate::ui::UserInterface {
                tile_tree: Some(tile_tree),
                tile_tree_context,
                ..
            } = &mut context.resources.user_interface
            else {
                return;
            };

            // Update mappings using free function
            let mut counter = 0;
            if let Some(root) = tile_tree.root {
                update_tile_mappings(
                    &tile_tree.tiles,
                    root,
                    &mut tile_tree_context.tile_mapping,
                    &mut counter,
                );
            }

            tile_tree.ui(tile_tree_context, ui);

            if let Some(parent) = tile_tree_context.add_child_to.take() {
                let new_child = tile_tree.tiles.insert_pane(Pane {
                    kind: PaneKind::Color(egui::Color32::from_rgb(200, 200, 200)),
                });
                if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) =
                    tile_tree.tiles.get_mut(parent)
                {
                    tabs.add_child(new_child);
                    tabs.set_active(new_child);
                }
            }
        });
}

fn right_panel_ui(context: &mut crate::context::Context, ui: &egui::Context) {
    if !context.resources.user_interface.show_right_panel {
        return;
    }
    egui::SidePanel::right("right").show(ui, |ui| {
        ui.label("Properties");
        ui.separator();
        ui.available_width();
        local_transform_inspector_ui(context, ui);
        camera_inspector_ui(context, ui);
        lines_inspector_ui(context, ui);
        quads_inspector_ui(context, ui);
        let time = context.resources.window.uptime_milliseconds;
        if let Some(selected_entity) = context.resources.user_interface.selected_entity {
            if ui.button("Paint").clicked() {
                let mut painting = crate::paint::Painting::default();
                paint_cube_scene(time as _, &mut painting);
                paint_entity(context, selected_entity, painting);
            }
        }
    });
}

fn bottom_panel_ui(context: &mut crate::context::Context, ui: &egui::Context) {
    if !context.resources.user_interface.show_bottom_panel {
        return;
    }

    egui::TopBottomPanel::bottom("timeline").show(ui, |ui| {
        timeline_ui(ui, &mut context.resources.user_interface.timeline_state);
    });

    if context.resources.user_interface.timeline_state.playing {
        ui.request_repaint();
    }
}

fn lines_inspector_ui(context: &mut crate::context::Context, ui: &mut egui::Ui) {
    use crate::context::*;

    let Some(entity) = context.resources.user_interface.selected_entity else {
        return;
    };

    ui.group(|ui| {
        ui.label("Lines");
        match get_component_mut::<Lines>(context, entity, LINES) {
            Some(Lines(lines)) => {
                let mut lines_to_remove = Vec::new();
                for (index, line) in lines.iter_mut().enumerate() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(format!("Line {}", index));
                            if ui.button("Remove").clicked() {
                                lines_to_remove.push(index);
                            }
                        });

                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label("Start:");
                                ui.label("x");
                                ui.add(egui::DragValue::new(&mut line.start.x).speed(0.1));
                                ui.label("y");
                                ui.add(egui::DragValue::new(&mut line.start.y).speed(0.1));
                                ui.label("z");
                                ui.add(egui::DragValue::new(&mut line.start.z).speed(0.1));
                            });
                        });

                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label("End:");
                                ui.label("x");
                                ui.add(egui::DragValue::new(&mut line.end.x).speed(0.1));
                                ui.label("y");
                                ui.add(egui::DragValue::new(&mut line.end.y).speed(0.1));
                                ui.label("z");
                                ui.add(egui::DragValue::new(&mut line.end.z).speed(0.1));
                            });
                        });

                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label("Color:");
                                let mut color = egui::Color32::from_rgba_unmultiplied(
                                    (line.color.x * 255.0) as u8,
                                    (line.color.y * 255.0) as u8,
                                    (line.color.z * 255.0) as u8,
                                    (line.color.w * 255.0) as u8,
                                );
                                if ui.color_edit_button_srgba(&mut color).changed() {
                                    line.color.x = color.r() as f32 / 255.0;
                                    line.color.y = color.g() as f32 / 255.0;
                                    line.color.z = color.b() as f32 / 255.0;
                                    line.color.w = color.a() as f32 / 255.0;
                                }
                            });
                        });
                    });
                }

                // Remove any lines marked for deletion (in reverse order to maintain indices)
                for index in lines_to_remove.into_iter().rev() {
                    lines.remove(index);
                }

                // Add new line button
                if ui.button("Add Line").clicked() {
                    lines.push(Line {
                        start: nalgebra_glm::vec3(0.0, 0.0, 0.0),
                        end: nalgebra_glm::vec3(1.0, 1.0, 1.0),
                        color: nalgebra_glm::vec4(1.0, 1.0, 1.0, 1.0),
                    });
                }

                if ui.button("Remove").clicked() {
                    remove_components(context, entity, LINES);
                }
            }
            None => {
                if ui.button("Add Lines").clicked() {
                    add_components(context, entity, LINES);
                    if let Some(Lines(lines)) = get_component_mut::<Lines>(context, entity, LINES) {
                        *lines = vec![Line {
                            start: nalgebra_glm::vec3(0.0, 0.0, 0.0),
                            end: nalgebra_glm::vec3(1.0, 1.0, 1.0),
                            color: nalgebra_glm::vec4(1.0, 1.0, 1.0, 1.0),
                        }];
                    }
                }
            }
        }
    });
}

pub fn quads_inspector_ui(context: &mut crate::context::Context, ui: &mut egui::Ui) {
    use crate::context::*;

    let Some(entity) = context.resources.user_interface.selected_entity else {
        return;
    };

    ui.group(|ui| {
        ui.label("Quads");
        match get_component_mut::<Quads>(context, entity, QUADS) {
            Some(Quads(quads)) => {
                // Show existing quads with edit/delete capabilities
                let mut quads_to_remove = Vec::new();
                for (index, quad) in quads.iter_mut().enumerate() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(format!("Quad {}", index));
                            if ui.button("Remove").clicked() {
                                quads_to_remove.push(index);
                            }
                        });

                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label("Size:");
                                ui.label("width");
                                ui.add(egui::DragValue::new(&mut quad.size.x).speed(0.1));
                                ui.label("height");
                                ui.add(egui::DragValue::new(&mut quad.size.y).speed(0.1));
                            });
                        });

                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label("Offset:");
                                ui.label("x");
                                ui.add(egui::DragValue::new(&mut quad.offset.x).speed(0.1));
                                ui.label("y");
                                ui.add(egui::DragValue::new(&mut quad.offset.y).speed(0.1));
                                ui.label("z");
                                ui.add(egui::DragValue::new(&mut quad.offset.z).speed(0.1));
                            });
                        });

                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label("Color:");
                                let mut color = egui::Color32::from_rgba_unmultiplied(
                                    (quad.color.x * 255.0) as u8,
                                    (quad.color.y * 255.0) as u8,
                                    (quad.color.z * 255.0) as u8,
                                    (quad.color.w * 255.0) as u8,
                                );
                                if ui.color_edit_button_srgba(&mut color).changed() {
                                    quad.color.x = color.r() as f32 / 255.0;
                                    quad.color.y = color.g() as f32 / 255.0;
                                    quad.color.z = color.b() as f32 / 255.0;
                                    quad.color.w = color.a() as f32 / 255.0;
                                }
                            });
                        });
                    });
                }

                // Remove any quads marked for deletion (in reverse order to maintain indices)
                for index in quads_to_remove.into_iter().rev() {
                    quads.remove(index);
                }

                // Add new quad button
                if ui.button("Add Quad").clicked() {
                    quads.push(Quad {
                        size: nalgebra_glm::vec2(1.0, 1.0),
                        offset: nalgebra_glm::vec3(0.0, 0.0, 0.0),
                        color: nalgebra_glm::vec4(1.0, 1.0, 1.0, 1.0),
                    });
                }

                if ui.button("Remove Component").clicked() {
                    remove_components(context, entity, QUADS);
                }
            }
            None => {
                if ui.button("Add Quads").clicked() {
                    add_components(context, entity, QUADS);
                    if let Some(Quads(quads)) = get_component_mut::<Quads>(context, entity, QUADS) {
                        *quads = vec![Quad {
                            size: nalgebra_glm::vec2(1.0, 1.0),
                            offset: nalgebra_glm::vec3(0.0, 0.0, 0.0),
                            color: nalgebra_glm::vec4(1.0, 1.0, 1.0, 1.0),
                        }];
                    }
                }
            }
        }
    });
}

fn camera_inspector_ui(context: &mut crate::context::Context, ui: &mut egui::Ui) {
    use crate::context::*;

    let Some(selected_entity) = context.resources.user_interface.selected_entity else {
        return;
    };

    ui.group(|ui| {
        ui.label("Camera");
        if let Some(camera) = get_component_mut::<Camera>(context, selected_entity, CAMERA) {
            ui.horizontal(|ui| {
                ui.label("FOV:");
                ui.add(egui::Slider::new(&mut camera.fov, 1.0..=120.0).suffix("°"));
            });

            if ui.button("Remove").clicked() {
                remove_components(context, selected_entity, CAMERA);
            }
        }
    });
}

fn left_panel_ui(context: &mut crate::context::Context, ui: &egui::Context) {
    if !context.resources.user_interface.show_left_panel {
        return;
    }
    egui::SidePanel::left("left").show(ui, |ui| {
        ui.label("Scene");
        ui.separator();
        ui.available_width();
        egui::ScrollArea::vertical()
            .id_salt(ui.next_auto_id())
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(ui.next_auto_id())
                    .show(ui, |ui| {
                        ui.group(|ui| {
                            if ui.button("Add Entity").clicked() {
                                let entity = crate::context::spawn_entities(
                                    context,
                                    crate::context::LOCAL_TRANSFORM
                                        | crate::context::GLOBAL_TRANSFORM,
                                    1,
                                )[0];
                                context.resources.user_interface.selected_entity = Some(entity);
                            }
                            crate::context::query_root_nodes(context)
                                .into_iter()
                                .for_each(|entity| {
                                    entity_tree_ui(context, ui, entity);
                                });
                        });
                    });

                ui.separator();
            });

        ui.group(|ui| {
            ui.label("Commands");

            if ui.button("Spawn Cube").clicked() {
                push_command(
                    context,
                    Command::Entity {
                        command: EntityCommand::SpawnCube {
                            position: nalgebra_glm::vec3(0.0, 0.0, 0.0),
                            size: 1.0,
                            name: "Cube".to_string(),
                        },
                    },
                );
            }

            if ui.button("Spawn Camera").clicked() {
                push_command(
                    context,
                    Command::Entity {
                        command: EntityCommand::SpawnCamera {
                            position: nalgebra_glm::vec3(0.0, 0.0, 5.0),
                            name: "Camera".to_string(),
                        },
                    },
                );
            }

            if ui.button("List Cameras").clicked() {
                push_command(
                    context,
                    Command::Request {
                        command: RequestCommand::RequestCameraEntities,
                    },
                );
            }

            ui.group(|ui| {
                let network_connected = context.resources.network.is_connected;
                if ui
                    .add_enabled(!network_connected, egui::Button::new("Connect Websocket"))
                    .clicked()
                {
                    let url = context
                        .resources
                        .user_interface
                        .backend_websocket_address
                        .to_string();
                    push_command(
                        context,
                        Command::Network {
                            command: NetworkCommand::Connect { url },
                        },
                    );
                }
                if context
                    .resources
                    .user_interface
                    .backend_websocket_address
                    .is_empty()
                {
                    context.resources.user_interface.backend_websocket_address =
                        "127.0.0.1:9001".to_string();
                }
                ui.text_edit_singleline(
                    &mut context.resources.user_interface.backend_websocket_address,
                );
            });

            if ui
                .add_enabled(
                    context.resources.network.is_connected,
                    egui::Button::new("Publish Message"),
                )
                .clicked()
            {
                push_command(
                    context,
                    Command::Network {
                        command: NetworkCommand::Send {
                            message: NetworkMessage::Text {
                                string: "Hello, from the nightshade frontend!".to_string(),
                            },
                        },
                    },
                );
            }

            if ui
                .add_enabled(
                    context.resources.network.is_connected,
                    egui::Button::new("Disconnect"),
                )
                .clicked()
            {
                push_command(
                    context,
                    Command::Network {
                        command: NetworkCommand::Disconnect,
                    },
                );
            }
        });
    });
}

fn top_panel_ui(context: &mut crate::context::Context, ui: &egui::Context) {
    egui::TopBottomPanel::top("menu").show(ui, |ui| {
        egui::menu::bar(ui, |ui| {
            egui::global_theme_preference_switch(ui);
            ui.separator();
            ui.label(format!(
                "FPS: {}",
                context.resources.window.frames_per_second
            ));
            ui.separator();
            ui.label("Panels:");
            ui.checkbox(
                &mut context.resources.user_interface.show_left_panel,
                "Left",
            );
            ui.checkbox(
                &mut context.resources.user_interface.show_right_panel,
                "Right",
            );
            ui.checkbox(
                &mut context.resources.user_interface.show_bottom_panel,
                "Bottom",
            );
            ui.separator();
        });
    });
}

// Recursively renders the entity tree in the ui system
fn entity_tree_ui(
    context: &mut crate::context::Context,
    ui: &mut egui::Ui,
    entity: crate::context::EntityId,
) {
    use crate::context::*;

    let name = match crate::context::get_component::<Name>(context, entity, NAME) {
        Some(Name(name)) if !name.is_empty() => name.to_string(),
        _ => "Entity".to_string(),
    };

    let selected = context.resources.user_interface.selected_entity == Some(entity);

    let id = ui.make_persistent_id(ui.next_auto_id());
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                let prefix = "🔵".to_string();
                let response = ui.selectable_label(selected, format!("{prefix}{name}"));

                if response.clicked() {
                    context.resources.user_interface.selected_entity = Some(entity);
                }

                response.context_menu(|ui| {
                    if ui.button("Add Child").clicked() {
                        let child =
                            spawn_entities(context, PARENT | LOCAL_TRANSFORM | GLOBAL_TRANSFORM, 1)
                                [0];
                        if let Some(parent) = get_component_mut::<Parent>(context, child, PARENT) {
                            *parent = Parent(entity);
                        }
                        ui.close_menu();
                    }
                    if ui.button("Remove").clicked() {
                        despawn_entities(context, &[entity]);
                        let descendents = query_descendents(context, entity);
                        for entity in descendents {
                            despawn_entities(context, &[entity]);
                        }
                        context.resources.user_interface.selected_entity = None;
                        ui.close_menu();
                    }
                });
            });
        })
        .body(|ui| {
            query_children(context, entity)
                .into_iter()
                .for_each(|child| {
                    entity_tree_ui(context, ui, child);
                });
        });
}

fn local_transform_inspector_ui(context: &mut crate::context::Context, ui: &mut egui::Ui) {
    use crate::context::*;
    let Some(selected_entity) = context.resources.user_interface.selected_entity else {
        return;
    };
    let mut uniform_scaling = context.resources.user_interface.uniform_scaling;
    ui.group(|ui| {
        match get_component_mut::<LocalTransform>(context, selected_entity, LOCAL_TRANSFORM) {
            Some(local_transform) => {
                ui.group(|ui| {
                    ui.label("Translation");
                    ui.horizontal(|ui| {
                        ui.label("x");
                        ui.add(egui::DragValue::new(&mut local_transform.translation.x).speed(0.1));
                        ui.label("y");
                        ui.add(egui::DragValue::new(&mut local_transform.translation.y).speed(0.1));
                        ui.label("z");
                        ui.add(egui::DragValue::new(&mut local_transform.translation.z).speed(0.1));
                    });
                });
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label("Scale");
                        ui.label("X");
                        if ui
                            .add(
                                egui::DragValue::new(&mut local_transform.scale.x)
                                    .speed(0.1)
                                    .range(0..=usize::MAX),
                            )
                            .changed()
                            && uniform_scaling
                        {
                            local_transform.scale.y = local_transform.scale.x;
                            local_transform.scale.z = local_transform.scale.x;
                        }
                        ui.label("Y");
                        if ui
                            .add(
                                egui::DragValue::new(&mut local_transform.scale.y)
                                    .speed(0.1)
                                    .range(0..=usize::MAX),
                            )
                            .changed()
                            && uniform_scaling
                        {
                            local_transform.scale.x = local_transform.scale.y;
                            local_transform.scale.z = local_transform.scale.y;
                        }
                        ui.label("Z");
                        if ui
                            .add(
                                egui::DragValue::new(&mut local_transform.scale.z)
                                    .speed(0.1)
                                    .range(0..=usize::MAX),
                            )
                            .changed()
                            && uniform_scaling
                        {
                            local_transform.scale.x = local_transform.scale.z;
                            local_transform.scale.y = local_transform.scale.z;
                        }
                        ui.separator();
                        ui.checkbox(&mut uniform_scaling, "Uniform");
                    });
                });
                ui.separator();
                if ui.button("Remove").clicked() {
                    remove_components(context, selected_entity, LOCAL_TRANSFORM);
                }
            }
            None => {
                if ui.button("Add Transform").clicked() {
                    add_components(context, selected_entity, LOCAL_TRANSFORM);
                }
            }
        }
    });

    context.resources.user_interface.uniform_scaling = uniform_scaling;
}

fn update_tile_mappings(
    tiles: &egui_tiles::Tiles<Pane>,
    tile_id: egui_tiles::TileId,
    mapping: &mut std::collections::HashMap<egui_tiles::TileId, usize>,
    counter: &mut usize,
) {
    // If this is a pane, assign it an ID
    if tiles
        .get(tile_id)
        .is_some_and(|t| matches!(t, egui_tiles::Tile::Pane(_)))
    {
        mapping.insert(tile_id, *counter);
        *counter += 1;
        return;
    }

    // For containers, process their children
    if let Some(egui_tiles::Tile::Container(container)) = tiles.get(tile_id) {
        match container {
            egui_tiles::Container::Tabs(tabs) => {
                for &child in &tabs.children {
                    update_tile_mappings(tiles, child, mapping, counter);
                }
            }
            egui_tiles::Container::Linear(linear) => {
                for &child in &linear.children {
                    update_tile_mappings(tiles, child, mapping, counter);
                }
            }
            egui_tiles::Container::Grid(grid) => {
                for child in grid.children() {
                    update_tile_mappings(tiles, *child, mapping, counter);
                }
            }
        }
    }
}

pub use timeline::*;
mod timeline {
    pub fn timeline_ui(ui: &mut egui::Ui, state: &mut TimelineState) {
        let rect = ui.max_rect();
        let border_color = egui::Color32::from_rgb(59, 130, 246);
        let border_width = 1.0;
        let border_rounding = 1.0;
        let border_rect = rect.shrink(1.0);
        ui.painter().rect_stroke(
            border_rect,
            border_rounding,
            egui::Stroke::new(border_width, border_color),
        );

        egui::Frame::none()
            .inner_margin(egui::Margin::from(6.0))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    // Time display
                    let time_text = format!(
                        "{} / {}",
                        format_time(state.current_time),
                        format_time(state.total_duration)
                    );
                    ui.label(time_text);

                    ui.add_space(16.0);

                    // Timeline slider
                    let slider =
                        egui::Slider::new(&mut state.current_time, 0.0..=state.total_duration)
                            .show_value(false)
                            .custom_formatter(|_, _| String::new())
                            .custom_parser(|s| s.parse::<f64>().ok());

                    if ui.add(slider).dragged() {
                        state.playing = false;
                    }

                    ui.add_space(16.0);

                    // Step buttons
                    if ui.button("⏮").clicked() {
                        state.current_time = (state.current_time - 1.0).max(0.0);
                    }

                    // Play/Pause button
                    if ui.button(if state.playing { "⏸" } else { "▶" }).clicked() {
                        state.playing = !state.playing;
                    }

                    if ui.button("⏭").clicked() {
                        state.current_time = (state.current_time + 1.0).min(state.total_duration);
                    }

                    ui.add_space(16.0);

                    egui::ComboBox::from_label("Speed")
                        .selected_text(format!("{:.1}x", state.playback_speed))
                        .show_ui(ui, |ui| {
                            for &speed in &[0.1, 0.5, 1.0, 2.0, 5.0] {
                                ui.selectable_value(
                                    &mut state.playback_speed,
                                    speed,
                                    format!("{:.1}x", speed),
                                );
                            }
                        });
                });
            });
    }

    #[derive(Debug, Clone)]
    pub struct TimelineState {
        pub playing: bool,
        pub current_time: f64,
        pub total_duration: f64,
        pub playback_speed: f64,
    }

    impl Default for TimelineState {
        fn default() -> Self {
            Self {
                playing: false,
                current_time: 0.0,
                total_duration: 100.0,
                playback_speed: 1.0,
            }
        }
    }

    pub fn update_timeline_system(context: &mut crate::context::Context) {
        let delta_time = context.resources.window.delta_time as f64;
        let timeilne = &mut context.resources.user_interface.timeline_state;
        if !timeilne.playing {
            return;
        }
        timeilne.current_time += delta_time * timeilne.playback_speed;
        if timeilne.current_time >= timeilne.total_duration {
            timeilne.current_time = timeilne.total_duration;
            timeilne.playing = false;
        }
    }

    pub fn format_time(seconds: f64) -> String {
        let minutes = (seconds as i32) / 60;
        let secs = (seconds as i32) % 60;
        let ms = (seconds.fract() * 1000.0) as i32;
        format!("{:02}:{:02}.{:03}", minutes, secs, ms)
    }
}
