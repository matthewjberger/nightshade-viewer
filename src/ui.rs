use crate::paint::{paint_cube_scene, paint_entity};

#[derive(Default)]
pub struct UserInterface {
    pub state: Option<egui_winit::State>,
    pub tile_tree: Option<egui_tiles::Tree<Pane>>,
    pub tile_tree_context: TileTreeContext,
    pub frame_output: Option<(egui::FullOutput, Vec<egui::ClippedPrimitive>)>,
    pub show_left_panel: bool,
    pub show_right_panel: bool,
    pub uniform_scaling: bool,
    pub consumed_event: bool,
    pub selected_entity: Option<crate::scene::EntityId>,
}

#[derive(Debug, Copy, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum PaneKind {
    MainCamera,
    Color(egui::Color32),
}

impl Default for PaneKind {
    fn default() -> Self {
        Self::MainCamera
    }
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Pane {
    pub kind: PaneKind,
}

/// A context shared between all the panes in the tile tree
#[derive(Default)]
pub struct TileTreeContext {
    pub tile_rects: std::collections::HashMap<egui_tiles::TileId, egui::Rect>,
    pub add_child_to: Option<egui_tiles::TileId>,
    pub viewport_tiles: std::collections::HashMap<egui_tiles::TileId, egui::Rect>,
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

    fn tab_title_for_pane(&mut self, _pane: &crate::ui::Pane) -> egui::WidgetText {
        "Pane".into()
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
        let rect = ui.max_rect();

        if matches!(pane.kind, PaneKind::MainCamera) {
            self.viewport_tiles.insert(tile_id, rect);
        }

        // Apply background color if set
        match pane.kind {
            PaneKind::Color(color) => {
                ui.painter().rect_filled(rect, 0.0, color);
            }
            PaneKind::MainCamera => {}
        }

        let mut drag_response = egui_tiles::UiResponse::None;

        // Create a top area for controls that won't interfere with dragging
        let controls_height = 28.0;
        let (controls_rect, content_rect) =
            rect.split_top_bottom_at_y(rect.min.y + controls_height);

        // Handle controls in the top area
        let controls_response = ui.allocate_ui_with_layout(
            controls_rect.size(),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(4.0);

                egui::ComboBox::new(format!("background_{}", tile_id.0), "")
                    .selected_text(match pane.kind {
                        PaneKind::MainCamera => "Main Camera",
                        PaneKind::Color(_) => "Color",
                    })
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        let is_transparent = matches!(pane.kind, PaneKind::MainCamera);
                        if ui.selectable_label(is_transparent, "Main Camera").clicked() {
                            pane.kind = PaneKind::MainCamera;
                        }

                        let is_color = matches!(pane.kind, PaneKind::Color(_));
                        if ui.selectable_label(is_color, "Color").clicked() {
                            if let PaneKind::MainCamera = pane.kind {
                                pane.kind = PaneKind::Color(egui::Color32::from_gray(200));
                            }
                        }
                    });

                if let PaneKind::Color(ref mut color) = pane.kind {
                    ui.add_space(4.0);
                    ui.color_edit_button_srgba(color);
                }
            },
        );

        // Handle the content area
        let _content_response = ui.allocate_ui_with_layout(
            content_rect.size(),
            egui::Layout::centered_and_justified(egui::Direction::TopDown),
            |ui| {
                ui.label(format!("Tile {}", tile_id.0));
            },
        );

        // Only enable dragging in the content area and when shift is pressed
        let shift_pressed = ui.input(|i| i.modifiers.shift);
        if shift_pressed && !controls_response.response.hovered() {
            let cursor = egui::CursorIcon::Grab;
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

pub fn receive_ui_event(context: &mut crate::scene::Context, event: &winit::event::WindowEvent) {
    let Some(gui_state) = &mut context.resources.user_interface.state else {
        return;
    };
    let Some(window_handle) = context.resources.window.handle.as_ref() else {
        return;
    };
    context.resources.user_interface.consumed_event =
        gui_state.on_window_event(window_handle, event).consumed;
}

/// Resizes the egui UI, ensuring it matches the window scale factor
pub fn resize_ui(context: &mut crate::scene::Context) {
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
pub fn ensure_tile_tree_system(context: &mut crate::scene::Context) {
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
pub fn render_ui_system(context: &mut crate::scene::Context) {
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

    let output = ui.end_pass();
    let paint_jobs = ui.tessellate(output.shapes.clone(), output.pixels_per_point);
    context.resources.user_interface.frame_output = Some((output, paint_jobs));
}

fn create_ui(context: &mut crate::scene::Context, ui: &egui::Context) {
    top_panel_ui(context, ui);
    left_panel_ui(context, ui);
    right_panel_ui(context, ui);
    central_panel_ui(context, ui);
}

fn central_panel_ui(context: &mut crate::scene::Context, ui: &egui::Context) {
    egui::CentralPanel::default()
        .frame(egui::Frame::none())
        .show(ui, |ui| {
            context
                .resources
                .user_interface
                .tile_tree_context
                .viewport_tiles
                .clear();
            let crate::ui::UserInterface {
                tile_tree: Some(tile_tree),
                tile_tree_context,
                ..
            } = &mut context.resources.user_interface
            else {
                return;
            };
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

fn right_panel_ui(context: &mut crate::scene::Context, ui: &egui::Context) {
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

pub fn lines_inspector_ui(context: &mut crate::scene::Context, ui: &mut egui::Ui) {
    use crate::scene::*;

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
                                ui.label("r");
                                ui.add(
                                    egui::DragValue::new(&mut line.color.x)
                                        .speed(0.1)
                                        .range(0.0..=1.0),
                                );
                                ui.label("g");
                                ui.add(
                                    egui::DragValue::new(&mut line.color.y)
                                        .speed(0.1)
                                        .range(0.0..=1.0),
                                );
                                ui.label("b");
                                ui.add(
                                    egui::DragValue::new(&mut line.color.z)
                                        .speed(0.1)
                                        .range(0.0..=1.0),
                                );
                                ui.label("a");
                                ui.add(
                                    egui::DragValue::new(&mut line.color.w)
                                        .speed(0.1)
                                        .range(0.0..=1.0),
                                );
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

pub fn quads_inspector_ui(context: &mut crate::scene::Context, ui: &mut egui::Ui) {
    use crate::scene::*;

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
                                ui.label("r");
                                ui.add(
                                    egui::DragValue::new(&mut quad.color.x)
                                        .speed(0.1)
                                        .range(0.0..=1.0),
                                );
                                ui.label("g");
                                ui.add(
                                    egui::DragValue::new(&mut quad.color.y)
                                        .speed(0.1)
                                        .range(0.0..=1.0),
                                );
                                ui.label("b");
                                ui.add(
                                    egui::DragValue::new(&mut quad.color.z)
                                        .speed(0.1)
                                        .range(0.0..=1.0),
                                );
                                ui.label("a");
                                ui.add(
                                    egui::DragValue::new(&mut quad.color.w)
                                        .speed(0.1)
                                        .range(0.0..=1.0),
                                );
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

fn camera_inspector_ui(context: &mut crate::scene::Context, ui: &mut egui::Ui) {
    use crate::scene::*;

    let (viewport_width, viewport_height) = context.resources.graphics.viewport_size;
    let Some(selected_entity) = context.resources.user_interface.selected_entity else {
        return;
    };

    ui.group(|ui| {
        ui.label("Camera");
        if let Some(camera) = get_component_mut::<Camera>(context, selected_entity, CAMERA) {
            if let Some(viewport) = camera.viewport.as_mut() {
                ui.group(|ui| {
                    ui.label("Viewport");
                    ui.horizontal(|ui| {
                        ui.label("x");
                        ui.add(egui::DragValue::new(&mut viewport.x).speed(0.1));
                        ui.label("y");
                        ui.add(egui::DragValue::new(&mut viewport.y).speed(0.1));
                        ui.label("width");
                        ui.add(egui::DragValue::new(&mut viewport.width).speed(0.1));
                        ui.label("height");
                        ui.add(egui::DragValue::new(&mut viewport.height).speed(0.1));
                    });
                });
                if ui.button("Remove Viewport").clicked() {
                    camera.viewport = None;
                }
            } else if ui.button("Add Viewport").clicked() {
                camera.viewport = Some(Viewport {
                    x: 0,
                    y: 0,
                    width: viewport_width,
                    height: viewport_height,
                });
            }

            match &camera.projection {
                Projection::Perspective(_perspective_camera) => {
                    ui.label("Projection is `Perspective`");
                }
                Projection::Orthographic(_orthographic_camera) => {
                    ui.label("Projection is `Orthographic`");
                }
            }

            if ui.button("Remove").clicked() {
                remove_components(context, selected_entity, CAMERA);
                context.resources.user_interface.selected_entity = None;
            }
        } else if ui.button("Add").clicked() {
            add_components(context, selected_entity, CAMERA);
        }
    });
}

fn left_panel_ui(context: &mut crate::scene::Context, ui: &egui::Context) {
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
                                let entity = crate::scene::spawn_entities(
                                    context,
                                    crate::scene::LOCAL_TRANSFORM | crate::scene::GLOBAL_TRANSFORM,
                                    1,
                                )[0];
                                context.resources.user_interface.selected_entity = Some(entity);
                            }
                            crate::scene::query_root_nodes(context)
                                .into_iter()
                                .for_each(|entity| {
                                    entity_tree_ui(context, ui, entity);
                                });
                        });
                    });

                ui.separator();
            });
    });
}

fn top_panel_ui(context: &mut crate::scene::Context, ui: &egui::Context) {
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
            )
            .on_hover_text("Toggle Left Panel");
            ui.checkbox(
                &mut context.resources.user_interface.show_right_panel,
                "Right",
            )
            .on_hover_text("Toggle Right Panel");
            ui.separator();
        });
    });
}

// Recursively renders the entity tree in the ui system
fn entity_tree_ui(
    context: &mut crate::scene::Context,
    ui: &mut egui::Ui,
    entity: crate::scene::EntityId,
) {
    use crate::scene::*;

    let name = match crate::scene::get_component::<Name>(context, entity, NAME) {
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

fn local_transform_inspector_ui(context: &mut crate::scene::Context, ui: &mut egui::Ui) {
    use crate::scene::*;
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
