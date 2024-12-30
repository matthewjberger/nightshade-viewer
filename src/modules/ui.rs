#[derive(Default)]
pub struct UserInterface {
    pub state: Option<egui_winit::State>,
    pub tile_tree: Option<egui_tiles::Tree<Pane>>,
    pub tile_tree_context: TileTreeContext,
    pub frame_output: Option<(egui::FullOutput, Vec<egui::ClippedPrimitive>)>,
    pub show_left_panel: bool,
    pub show_right_panel: bool,
    pub consumed_event: bool,
    pub selected_entity: Option<crate::modules::scene::EntityId>,
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

impl egui_tiles::Behavior<crate::modules::ui::Pane> for crate::modules::ui::TileTreeContext {
    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        24.0
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        2.0
    }

    fn is_tab_closable(
        &self,
        _tiles: &egui_tiles::Tiles<crate::modules::ui::Pane>,
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

    fn tab_title_for_pane(&mut self, _pane: &crate::modules::ui::Pane) -> egui::WidgetText {
        "Pane".into()
    }

    fn top_bar_right_ui(
        &mut self,
        _tiles: &egui_tiles::Tiles<crate::modules::ui::Pane>,
        _ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        _tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
        _pane: &mut crate::modules::ui::Pane,
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

pub mod events {
    pub fn receive_ui_event(
        context: &mut crate::modules::scene::Context,
        event: &winit::event::WindowEvent,
    ) {
        let Some(gui_state) = &mut context.resources.user_interface.state else {
            return;
        };
        let Some(window_handle) = context.resources.window.handle.as_ref() else {
            return;
        };
        context.resources.user_interface.consumed_event =
            gui_state.on_window_event(window_handle, event).consumed;
    }
}

pub mod systems {
    /// Ensures a default layout when the tile tree is emptied
    pub fn ensure_tile_tree(context: &mut crate::modules::scene::Context) {
        if let Some(tile_tree) = &context.resources.user_interface.tile_tree {
            if !tile_tree.tiles.is_empty() {
                return;
            }
        }
        let mut tiles = egui_tiles::Tiles::default();
        let mut tab_tiles = vec![];
        let tab_tile_child = tiles.insert_pane(crate::modules::ui::Pane::default());
        let tab_tile = tiles.insert_tab_tile(vec![tab_tile_child]);
        tab_tiles.push(tab_tile);
        let root = tiles.insert_tab_tile(tab_tiles);
        let tiles = egui_tiles::Tree::new("tree", root, tiles);
        context.resources.user_interface.tile_tree = Some(tiles);
    }

    /// Creates the UI for the frame and
    /// emits the resources needed for rendering
    pub fn render_ui(context: &mut crate::modules::scene::Context) {
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
        egui::TopBottomPanel::top("menu").show(&ui, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::global_theme_preference_switch(ui);
                ui.separator();
                ui.menu_button("Project", |ui| {
                    let _ = ui.button("Save");
                    let _ = ui.button("Load");
                });
                ui.separator();
                ui.label(format!(
                    "FPS: {}",
                    context.resources.frame_timing.frames_per_second
                ));
                ui.separator();
                ui.checkbox(
                    &mut context.resources.user_interface.show_left_panel,
                    "Show Left Panel",
                );
                ui.checkbox(
                    &mut context.resources.user_interface.show_right_panel,
                    "Show Right Panel",
                );
                ui.separator();
            });
        });

        if context.resources.user_interface.show_left_panel {
            egui::SidePanel::left("left").show(&ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(ui.next_auto_id())
                    .show(ui, |ui| {
                        ui.collapsing("Scene", |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt(ui.next_auto_id())
                                .show(ui, |ui| {
                                    ui.group(|ui| {
                                        if ui.button("Create Entity").clicked() {
                                            let entity = crate::modules::scene::spawn_entities(
                                                context,
                                                crate::modules::scene::VISIBLE,
                                                1,
                                            )[0];
                                            context.resources.user_interface.selected_entity =
                                                Some(entity);
                                        }
                                        crate::modules::scene::queries::query_root_nodes(context)
                                            .into_iter()
                                            .for_each(|entity| {
                                                entity_tree_ui(context, ui, entity);
                                            });
                                    });
                                });
                        });
                        ui.separator();
                    });
            });
        }

        if context.resources.user_interface.show_right_panel {
            use crate::modules::scene::*;
            egui::SidePanel::right("right").show(&ui, |ui| {
                ui.collapsing("Properties", |ui| {
                    ui.group(|ui| {
                        ui.label("Camera");
                        if let Some(selected_entity) =
                            context.resources.user_interface.selected_entity
                        {
                            if let Some(camera) =
                                get_component_mut::<Camera>(context, selected_entity, CAMERA)
                            {
                                ui.group(|ui| {
                                    ui.label("Viewport");
                                    ui.horizontal(|ui| {
                                        ui.label("x");
                                        ui.add(
                                            egui::DragValue::new(&mut camera.viewport.x).speed(0.1),
                                        );
                                        ui.label("y");
                                        ui.add(
                                            egui::DragValue::new(&mut camera.viewport.y).speed(0.1),
                                        );
                                        ui.label("width");
                                        ui.add(
                                            egui::DragValue::new(&mut camera.viewport.width)
                                                .speed(0.1),
                                        );
                                        ui.label("height");
                                        ui.add(
                                            egui::DragValue::new(&mut camera.viewport.height)
                                                .speed(0.1),
                                        );
                                    });
                                });

                                if let Some(tile_id) = camera.tile_id.as_mut() {
                                    ui.group(|ui| {
                                        ui.label("Tile ID");
                                        ui.add(egui::DragValue::new(&mut tile_id.0).speed(0.1));
                                    });
                                } else if ui.button("Add Tile ID").clicked() {
                                    camera.tile_id = Some(egui_tiles::TileId(0));
                                }

                                match &camera.projection {
                                    Projection::Perspective(_perspective_camera) => {
                                        ui.label("Projection is `Perspective`");
                                    }
                                    Projection::Orthographic(_orthographic_camera) => {
                                        ui.label("Projection is `Orhographic`");
                                    }
                                }

                                if ui.button("Remove").clicked() {
                                    remove_components(context, selected_entity, CAMERA);
                                }
                            } else if ui.button("Add Camera").clicked() {
                                add_components(context, selected_entity, CAMERA);
                            }
                        }
                    });
                });
            });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(&ui, |ui| {
                let crate::modules::ui::UserInterface {
                    tile_tree: Some(tile_tree),
                    tile_tree_context,
                    ..
                } = &mut context.resources.user_interface
                else {
                    return;
                };
                tile_tree.ui(tile_tree_context, ui);
                if let Some(parent) = tile_tree_context.add_child_to.take() {
                    let new_child = tile_tree.tiles.insert_pane(crate::modules::ui::Pane {});
                    if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) =
                        tile_tree.tiles.get_mut(parent)
                    {
                        tabs.add_child(new_child);
                        tabs.set_active(new_child);
                    }
                }
            });

        let output = ui.end_pass();
        let paint_jobs = ui.tessellate(output.shapes.clone(), output.pixels_per_point);
        context.resources.user_interface.frame_output = Some((output, paint_jobs));
    }

    // Recursively renders the entity tree in the ui system
    fn entity_tree_ui(
        context: &mut crate::modules::scene::Context,
        ui: &mut egui::Ui,
        entity: crate::modules::scene::EntityId,
    ) {
        use crate::modules::scene::*;

        let name = match crate::modules::scene::get_component::<Name>(context, entity, NAME) {
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
                            let child = spawn_entities(context, PARENT | VISIBLE, 1)[0];
                            if let Some(parent) =
                                get_component_mut::<Parent>(context, child, PARENT)
                            {
                                *parent = Parent(entity);
                            }
                            ui.close_menu();
                        }
                        if ui.button("Remove").clicked() {
                            despawn_entities(context, &[entity]);
                            let descendents =
                                crate::modules::scene::queries::query_descendents(context, entity);
                            for entity in descendents {
                                despawn_entities(context, &[entity]);
                            }
                            ui.close_menu();
                        }
                    });
                });
            })
            .body(|ui| {
                crate::modules::scene::queries::query_children(context, entity)
                    .into_iter()
                    .for_each(|child| {
                        entity_tree_ui(context, ui, child);
                    });
            });
    }
}
