use crate::{
    api::{publish_command, publish_event, Message},
    prelude::*,
};

#[derive(Default)]
pub struct UserInterface {
    pub state: Option<egui_winit::State>,
    pub tile_tree: Option<egui_tiles::Tree<Pane>>,
    pub tile_tree_context: TileTreeContext,
    pub frame_output: Option<(egui::FullOutput, Vec<egui::ClippedPrimitive>)>,
    pub show_left_panel: bool,
    pub show_right_panel: bool,
    pub show_command_window: bool,
    pub uniform_scaling: bool,
    pub consumed_event: bool,
    pub selected_entity: Option<crate::context::EntityId>,
    pub backend_websocket_address: String,
    pub dragging_viewport: Option<(egui_tiles::TileId, egui::Pos2)>,
    pub api_log: Vec<ApiLogEntry>,
    pub draft_message: Message,
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
        scene_entity: EntityId,
        camera_entity: Option<EntityId>,
    },
    Color(egui::Color32),
    ApiLog,
    Empty,
}

impl Default for PaneKind {
    fn default() -> Self {
        Self::Empty
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

    fn tab_title_for_pane(
        &mut self, // required by egui_tiles
        pane: &crate::ui::Pane,
    ) -> egui::WidgetText {
        match pane.kind {
            PaneKind::Scene {
                scene_entity,
                camera_entity: _,
            } => {
                if let Some(context) = self.context.and_then(|ctx| unsafe { ctx.as_ref() }) {
                    if let Some(Name(name)) = get_component::<Name>(context, scene_entity, NAME) {
                        return format!("Scene: {}", name).into();
                    }
                }
                "Scene".into()
            }
            PaneKind::Color(_) => "Color".into(),
            PaneKind::ApiLog => "API Log".into(),
            PaneKind::Empty => "Empty".into(),
        }
    }

    fn top_bar_right_ui(
        &mut self, // required by egui_tiles
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
        &mut self, // required by egui_tiles
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
        pane: &mut crate::ui::Pane,
    ) -> egui_tiles::UiResponse {
        let rect = ui.max_rect();
        self.tile_rects.insert(tile_id, rect);

        if matches!(pane.kind, PaneKind::Scene { .. } | PaneKind::Color(_)) {
            self.viewport_tiles.insert(tile_id, (pane.kind, rect));
        }

        if let Some(Some(context)) = self.context.as_mut().map(|ctx| unsafe { ctx.as_mut() }) {
            // Add viewport controls at the top
            let controls_height = 28.0;
            let (controls_rect, viewport_rect) =
                rect.split_top_bottom_at_y(rect.min.y + controls_height);

            // Handle viewport dragging
            let response = ui
                .allocate_rect(viewport_rect, egui::Sense::click_and_drag())
                .on_hover_cursor(if ui.input(|i| i.modifiers.shift) {
                    egui::CursorIcon::Grab
                } else {
                    egui::CursorIcon::Default
                });

            // Check if shift is held
            let shift_held = ui.input(|i| i.modifiers.shift);

            // Return DragStarted if shift is held and dragging
            if shift_held && response.dragged() {
                if let Some(mouse_pos) = ui.ctx().pointer_latest_pos() {
                    context.resources.user_interface.dragging_viewport = Some((tile_id, mouse_pos));
                    return egui_tiles::UiResponse::DragStarted;
                }
            }

            // Handle ongoing drag
            if let Some((dragging_tile_id, initial_pos)) =
                context.resources.user_interface.dragging_viewport
            {
                if dragging_tile_id == tile_id {
                    if let Some(current_pos) = ui.ctx().pointer_latest_pos() {
                        // Stop dragging if shift is released or mouse button is released
                        if !shift_held || !ui.input(|i| i.pointer.primary_down()) {
                            context.resources.user_interface.dragging_viewport = None;
                        } else {
                            // Show grab cursor while dragging
                            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);

                            // Calculate drag delta and update viewport position
                            let delta = current_pos - initial_pos;
                            if delta.length() > 0.0 {
                                // Find a suitable drop target
                                for (other_tile_id, (_, other_rect)) in &self.viewport_tiles {
                                    if *other_tile_id != tile_id && other_rect.contains(current_pos)
                                    {
                                        // Get the tiles from the tree
                                        if let Some(tree) =
                                            &mut context.resources.user_interface.tile_tree
                                        {
                                            // First get the panes immutably
                                            let pane1 = if let Some(egui_tiles::Tile::Pane(pane)) =
                                                tree.tiles.get(tile_id)
                                            {
                                                Some(pane.clone())
                                            } else {
                                                None
                                            };

                                            let pane2 = if let Some(egui_tiles::Tile::Pane(pane)) =
                                                tree.tiles.get(*other_tile_id)
                                            {
                                                Some(pane.clone())
                                            } else {
                                                None
                                            };

                                            // Then do the swap if we got both panes
                                            if let (Some(pane1), Some(pane2)) = (pane1, pane2) {
                                                // Update first tile
                                                if let Some(tile1) = tree.tiles.get_mut(tile_id) {
                                                    *tile1 = egui_tiles::Tile::Pane(pane2);
                                                }
                                                // Update second tile
                                                if let Some(tile2) =
                                                    tree.tiles.get_mut(*other_tile_id)
                                                {
                                                    *tile2 = egui_tiles::Tile::Pane(pane1);
                                                }
                                                context
                                                    .resources
                                                    .user_interface
                                                    .dragging_viewport = None;
                                            }
                                        }
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            match pane.kind {
                PaneKind::ApiLog => {
                    // Draw dark background for entire pane area including controls
                    let bg_color = egui::Color32::from_gray(32);
                    ui.painter().rect_filled(rect, 0.0, bg_color);

                    // Create a child UI for the log content area with padding
                    let viewport_with_padding = viewport_rect.shrink(4.0);
                    let mut content_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(viewport_with_padding)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    );

                    // Show API log content
                    egui::ScrollArea::vertical().stick_to_bottom(true).show(
                        &mut content_ui,
                        |ui| {
                            // Create a frame for the entire log content
                            egui::Frame::NONE
                                .inner_margin(egui::Margin::symmetric(0, 0))
                                .show(ui, |ui| {
                                    // Show entries with alternating backgrounds
                                    for (idx, entry) in
                                        context.resources.user_interface.api_log.iter().enumerate()
                                    {
                                        let row_bg = if idx % 2 == 0 {
                                            egui::Color32::from_gray(28)
                                        } else {
                                            egui::Color32::from_gray(35)
                                        };

                                        // Create a frame for each row that spans the full width
                                        egui::Frame::NONE
                                            .fill(row_bg)
                                            .inner_margin(egui::Margin::symmetric(8, 4))
                                            .outer_margin(egui::Margin::symmetric(0, 0))
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    // Determine label and color based on Message variant
                                                    let (label, color) = match entry.message {
                                                        Message::Command { .. } => (
                                                            "COMMAND",
                                                            egui::Color32::from_rgb(130, 170, 255),
                                                        ),
                                                        Message::Event { .. } => (
                                                            "EVENT  ",
                                                            egui::Color32::from_rgb(130, 255, 170),
                                                        ),
                                                    };

                                                    // Show type label
                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(label)
                                                                .monospace()
                                                                .strong()
                                                                .color(color),
                                                        )
                                                        .wrap(),
                                                    );

                                                    // Show message content
                                                    ui.with_layout(
                                                        egui::Layout::left_to_right(
                                                            egui::Align::Center,
                                                        )
                                                        .with_cross_justify(true),
                                                        |ui| {
                                                            ui.label(
                                                                egui::RichText::new(format!(
                                                                    "{:?}",
                                                                    entry.message
                                                                ))
                                                                .monospace()
                                                                .color(egui::Color32::from_gray(
                                                                    230,
                                                                )),
                                                            );
                                                        },
                                                    );
                                                });
                                            });
                                    }
                                });
                        },
                    );
                }
                PaneKind::Empty => {
                    // Draw dark background for entire pane area
                    let bg_color = egui::Color32::from_gray(32);
                    ui.painter().rect_filled(rect, 0.0, bg_color);

                    // Show empty message
                    let text = "Select a visual from the dropdown above";
                    let text_color = egui::Color32::from_rgb(130, 255, 170);
                    let galley = ui.painter().layout_no_wrap(
                        text.into(),
                        egui::FontId::proportional(14.0),
                        text_color,
                    );

                    let text_rect = galley.rect;
                    let center = viewport_rect.center();
                    let text_pos = center - text_rect.size() * 0.5;
                    ui.painter().galley(text_pos, galley, text_color);
                }
                PaneKind::Scene { camera_entity, .. } => {
                    // Check if there are any cameras in the world
                    let cameras_exist = !query_entities(context, CAMERA).is_empty();

                    if !cameras_exist || camera_entity.is_none() {
                        // Draw dark background for entire pane area including controls
                        let bg_color = egui::Color32::from_gray(32);
                        ui.painter().rect_filled(
                            rect, // Use full pane rect
                            0.0,  // No rounding
                            bg_color,
                        );

                        // Show warning about missing camera
                        let warning_text = if !cameras_exist {
                            "No Cameras Available\n\nAdd a camera component to an entity in the scene tree."
                        } else {
                            "No Camera Selected\n\nSelect a camera from the dropdown above."
                        };

                        let warning_color = egui::Color32::from_rgb(220, 130, 0);
                        let galley = ui.painter().layout_no_wrap(
                            warning_text.into(),
                            egui::FontId::proportional(14.0),
                            warning_color,
                        );

                        let text_rect = galley.rect;
                        let center = viewport_rect.center();
                        let text_pos = center - text_rect.size() * 0.5;
                        ui.painter().galley(text_pos, galley, warning_color);
                    }
                }
                _ => {}
            }

            // Add controls UI after background
            let _child_response = ui.allocate_rect(controls_rect, egui::Sense::hover());
            let mut child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(controls_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );

            child_ui.horizontal(|ui| {
                ui.add_space(8.0);

                egui::ComboBox::new(format!("type_{}", tile_id.0), "")
                    .selected_text(match pane.kind {
                        PaneKind::Scene { .. } => "Scene",
                        PaneKind::Color(_) => "Color",
                        PaneKind::ApiLog => "API Log",
                        PaneKind::Empty => "Empty",
                    })
                    .show_ui(ui, |ui| {
                        let is_scene = matches!(pane.kind, PaneKind::Scene { .. });
                        if ui.selectable_label(is_scene, "Scene").clicked() && !is_scene {
                            // When switching to Scene, use existing root node if available
                            if let Some(existing_scene) = query_entities(context, LOCAL_TRANSFORM)
                                .into_iter()
                                .find(|e| get_component::<Parent>(context, *e, PARENT).is_none())
                            {
                                // Get the camera for this scene
                                let camera = query_children(context, existing_scene)
                                    .into_iter()
                                    .find(|e| {
                                        get_component::<Camera>(context, *e, CAMERA).is_some()
                                    });

                                pane.kind = PaneKind::Scene {
                                    scene_entity: existing_scene,
                                    camera_entity: camera,
                                };
                            } else {
                                // Create new root node if none exists
                                pane.kind = create_scene_pane(context).kind;
                            }
                        }

                        let is_color = matches!(pane.kind, PaneKind::Color(_));
                        if ui.selectable_label(is_color, "Color").clicked() && !is_color {
                            pane.kind = PaneKind::Color(egui::Color32::from_gray(200));
                        }

                        let is_api_log = matches!(pane.kind, PaneKind::ApiLog);
                        if ui.selectable_label(is_api_log, "API Log").clicked() && !is_api_log {
                            pane.kind = PaneKind::ApiLog;
                        }

                        let is_empty = matches!(pane.kind, PaneKind::Empty);
                        if ui.selectable_label(is_empty, "Empty").clicked() && !is_empty {
                            pane.kind = PaneKind::Empty;
                        }
                    });

                // Camera selector for Scene panes
                if let PaneKind::Scene {
                    scene_entity,
                    camera_entity,
                } = pane.kind
                {
                    // Get all cameras in the world
                    let all_cameras: Vec<_> = query_entities(context, CAMERA)
                        .into_iter()
                        .map(|camera| {
                            // Get camera name
                            let camera_name = if let Some(Name(name)) =
                                get_component::<Name>(context, camera, NAME)
                            {
                                name.clone()
                            } else {
                                format!("Camera {}", camera.id)
                            };

                            // Get scene name (parent's name)
                            let scene_name = if let Some(Parent(parent)) =
                                get_component::<Parent>(context, camera, PARENT)
                            {
                                if let Some(Name(name)) =
                                    get_component::<Name>(context, *parent, NAME)
                                {
                                    format!(" ({})", name)
                                } else {
                                    format!(" (Scene {})", parent.id)
                                }
                            } else {
                                " (No Scene)".to_string()
                            };

                            (camera, format!("{}{}", camera_name, scene_name))
                        })
                        .collect();

                    // Get current camera name for display with scene
                    let current_camera_text = if let Some(cam) = camera_entity {
                        let camera_name =
                            if let Some(Name(name)) = get_component::<Name>(context, cam, NAME) {
                                name.clone()
                            } else {
                                format!("Camera {}", cam.id)
                            };

                        let scene_name = if let Some(Parent(parent)) =
                            get_component::<Parent>(context, cam, PARENT)
                        {
                            if let Some(Name(name)) = get_component::<Name>(context, *parent, NAME)
                            {
                                format!(" ({})", name)
                            } else {
                                format!(" (Scene {})", parent.id)
                            }
                        } else {
                            " (No Scene)".to_string()
                        };

                        format!("{}{}", camera_name, scene_name)
                    } else {
                        "No Camera".to_string()
                    };

                    egui::ComboBox::from_label("")
                        .selected_text(current_camera_text)
                        .show_ui(ui, |ui| {
                            for (camera, label) in &all_cameras {
                                if ui
                                    .selectable_label(Some(*camera) == camera_entity, label)
                                    .clicked()
                                {
                                    // Create new PaneKind outside the closure
                                    let new_kind = PaneKind::Scene {
                                        scene_entity,
                                        camera_entity: Some(*camera),
                                    };
                                    pane.kind = new_kind;
                                }
                            }
                        });
                }
            });

            // Draw selection border
            if self.selected_tile == Some(tile_id) {
                let border_color = egui::Color32::from_rgb(251, 146, 60); // Orange color
                let border_width = 2.0;
                let border_rounding = 4.0;

                let border_rect = rect.shrink(1.0);
                ui.painter().rect_stroke(
                    border_rect,
                    border_rounding,
                    egui::Stroke::new(border_width, border_color),
                    egui::StrokeKind::Outside,
                );
            }

            // Handle viewport interaction
            let viewport_response = ui.allocate_rect(viewport_rect, egui::Sense::click());

            // Only handle viewport clicks if no color picker is open
            if viewport_response.clicked()
                && !ui.memory(|mem| mem.is_popup_open(egui::Id::new("color_picker")))
            {
                self.selected_tile = Some(tile_id);
                if let PaneKind::Scene {
                    camera_entity: Some(camera),
                    ..
                } = pane.kind
                {
                    context.resources.active_camera_entity = Some(camera);
                }
            }
        }

        egui_tiles::UiResponse::None
    }

    fn on_tab_close(
        &mut self, // required by egui_tiles
        tiles: &mut egui_tiles::Tiles<crate::ui::Pane>,
        tile_id: egui_tiles::TileId,
    ) -> bool {
        // Remove the tile and its associated data
        if let Some(egui_tiles::Tile::Pane(_)) = tiles.remove(tile_id) {
            // Clean up any viewport data for this tile
            self.viewport_tiles.remove(&tile_id);
            self.tile_rects.remove(&tile_id);
            self.tile_mapping.remove(&tile_id);
            if self.selected_tile == Some(tile_id) {
                self.selected_tile = None;
            }
            true // Indicate the tab was successfully closed
        } else {
            false // Indicate the tab wasn't closed
        }
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
                    scene_entity: _,
                    camera_entity,
                } => {
                    if let Some(camera) = camera_entity {
                        // Set both selected and active camera
                        context.resources.user_interface.selected_entity = Some(*camera);
                        context.resources.active_camera_entity = Some(*camera);
                    }
                }
                PaneKind::Color(_) => {}
                PaneKind::Empty => {}
                PaneKind::ApiLog => {}
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

    let tab_tile_child = tiles.insert_pane(Pane {
        kind: PaneKind::Empty,
    });
    let tab_tile = tiles.insert_tab_tile(vec![tab_tile_child]);
    tab_tiles.push(tab_tile);
    let root = tiles.insert_tab_tile(tab_tiles);
    let tiles = egui_tiles::Tree::new("tree", root, tiles);
    context.resources.user_interface.tile_tree = Some(tiles);
}

/// Creates the UI for the frame and
/// emits the resources needed for rendering
pub fn create_ui_system(context: &mut crate::context::Context) {
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
    central_panel_ui(context, ui);
}

fn central_panel_ui(context: &mut crate::context::Context, ui: &egui::Context) {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
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
                if matches!(
                    tile_tree_context
                        .context
                        .as_mut()
                        .map(|ctx| unsafe { ctx.as_mut() }),
                    Some(Some(_))
                ) {
                    let new_pane = Pane {
                        kind: PaneKind::Empty,
                    };

                    let new_child = tile_tree.tiles.insert_pane(new_pane);

                    if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) =
                        tile_tree.tiles.get_mut(parent)
                    {
                        tabs.add_child(new_child);
                        tabs.set_active(new_child);
                    }
                }
            }
        });
}

fn entity_inspector_ui(
    context: &mut crate::context::Context,
    ui: &mut egui::Ui,
    entity: crate::context::EntityId,
) {
    use crate::context::*;

    // Add Component Dropdown
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label("Add Component:");
            egui::ComboBox::new("add_component", "").show_ui(ui, |ui| {
                if get_component::<LocalTransform>(context, entity, LOCAL_TRANSFORM).is_none()
                    && ui.button("Transform").clicked()
                {
                    add_components(context, entity, LOCAL_TRANSFORM);
                }
                if get_component::<Camera>(context, entity, CAMERA).is_none()
                    && ui.button("Camera").clicked()
                {
                    add_components(context, entity, CAMERA);
                }
                if get_component::<Lines>(context, entity, LINES).is_none()
                    && ui.button("Lines").clicked()
                {
                    add_components(context, entity, LINES);
                }
                if get_component::<Quads>(context, entity, QUADS).is_none()
                    && ui.button("Quads").clicked()
                {
                    add_components(context, entity, QUADS);
                }
            });
        });
    });

    ui.separator();

    // Show existing components
    if get_component::<Name>(context, entity, NAME).is_some() {
        name_inspector_ui(context, ui, entity);
        ui.separator();
    }

    if get_component::<LocalTransform>(context, entity, LOCAL_TRANSFORM).is_some() {
        local_transform_inspector_ui(context, ui, entity);
        ui.separator();
    }

    if get_component::<Camera>(context, entity, CAMERA).is_some() {
        camera_inspector_ui(context, ui, entity);
        ui.separator();
    }

    if get_component::<Lines>(context, entity, LINES).is_some() {
        lines_inspector_ui(context, ui, entity);
        ui.separator();
    }

    if get_component::<Quads>(context, entity, QUADS).is_some() {
        quads_inspector_ui(context, ui, entity);
        ui.separator();
    }
}

fn name_inspector_ui(
    context: &mut crate::context::Context,
    ui: &mut egui::Ui,
    entity: crate::context::EntityId,
) {
    use crate::context::*;

    ui.group(|ui| {
        ui.label("Name");
        if let Some(Name(name)) = get_component_mut::<Name>(context, entity, NAME) {
            ui.text_edit_singleline(name);
            if ui.button("Remove Component").clicked() {
                remove_components(context, entity, NAME);
            }
        }
    });
}

fn lines_inspector_ui(
    context: &mut crate::context::Context,
    ui: &mut egui::Ui,
    entity: crate::context::EntityId,
) {
    use crate::context::*;

    ui.group(|ui| {
        ui.label("Lines");
        if let Some(Lines(lines)) = get_component_mut::<Lines>(context, entity, LINES) {
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
    });
}

pub fn quads_inspector_ui(
    context: &mut crate::context::Context,
    ui: &mut egui::Ui,
    entity: crate::context::EntityId,
) {
    use crate::context::*;

    ui.group(|ui| {
        ui.label("Quads");
        if let Some(Quads(quads)) = get_component_mut::<Quads>(context, entity, QUADS) {
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
    });
}

fn camera_inspector_ui(
    context: &mut crate::context::Context,
    ui: &mut egui::Ui,
    entity: crate::context::EntityId,
) {
    use crate::context::*;

    ui.group(|ui| {
        ui.label("Camera");
        if let Some(camera) = get_component_mut::<Camera>(context, entity, CAMERA) {
            // Projection type selector
            ui.horizontal(|ui| {
                ui.label("Projection:");
                let mut is_perspective = matches!(camera.projection, Projection::Perspective(_));
                if ui
                    .radio_value(&mut is_perspective, true, "Perspective")
                    .clicked()
                {
                    camera.projection = Projection::Perspective(PerspectiveCamera::default());
                }
                if ui
                    .radio_value(&mut is_perspective, false, "Orthographic")
                    .clicked()
                {
                    camera.projection = Projection::Orthographic(OrthographicCamera::default());
                }
            });

            // Projection-specific settings
            match &mut camera.projection {
                Projection::Perspective(perspective) => {
                    ui.horizontal(|ui| {
                        ui.label("FOV:");
                        ui.add(egui::Slider::new(&mut camera.fov, 1.0..=120.0).suffix("°"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Near:");
                        ui.add(egui::DragValue::new(&mut perspective.z_near).speed(0.1));
                    });
                    if let Some(far) = &mut perspective.z_far {
                        ui.horizontal(|ui| {
                            ui.label("Far:");
                            ui.add(egui::DragValue::new(far).speed(0.1));
                        });
                    }
                }
                Projection::Orthographic(ortho) => {
                    ui.horizontal(|ui| {
                        ui.label("Width:");
                        ui.add(egui::DragValue::new(&mut ortho.x_mag).speed(0.1));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Height:");
                        ui.add(egui::DragValue::new(&mut ortho.y_mag).speed(0.1));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Near:");
                        ui.add(egui::DragValue::new(&mut ortho.z_near).speed(0.1));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Far:");
                        ui.add(egui::DragValue::new(&mut ortho.z_far).speed(0.1));
                    });
                }
            }

            if ui.button("Remove Component").clicked() {
                remove_components(context, entity, CAMERA);
            }
        }
    });
}

fn left_panel_ui(context: &mut crate::context::Context, ui: &egui::Context) {
    if !context.resources.user_interface.show_left_panel {
        return;
    }
    egui::SidePanel::left("left").show(ui, |ui| {
        ui.available_width();
        egui::ScrollArea::vertical()
            .id_salt("left_panel_scroll")
            .show(ui, |ui| {
                // Scene Tree Section
                ui.collapsing("Scene Tree", |ui| {
                    // Add Scene button at top level
                    if ui.button("Add Scene").clicked() {
                        // Count existing root nodes for scene numbering
                        let scene_count = query_entities(context, LOCAL_TRANSFORM)
                            .into_iter()
                            .filter(|e| get_component::<Parent>(context, *e, PARENT).is_none())
                            .count();

                        let scene =
                            spawn_entities(context, NAME | LOCAL_TRANSFORM | GLOBAL_TRANSFORM, 1)
                                [0];

                        if let Some(name) = get_component_mut::<Name>(context, scene, NAME) {
                            *name = Name(format!("Scene {}", scene_count + 1));
                        }

                        // Create camera as child
                        let camera = spawn_entities(
                            context,
                            CAMERA | LOCAL_TRANSFORM | GLOBAL_TRANSFORM | NAME | PARENT,
                            1,
                        )[0];

                        if let Some(name) = get_component_mut::<Name>(context, camera, NAME) {
                            *name = Name(format!("Camera {}", scene_count + 1));
                        }

                        // Set up camera transform
                        initialize_camera_transform(context, camera);

                        // Parent camera to scene
                        if let Some(parent) = get_component_mut::<Parent>(context, camera, PARENT) {
                            *parent = Parent(scene);
                        }

                        context.resources.active_camera_entity = Some(camera);
                        context.resources.user_interface.selected_entity = Some(scene);
                    }

                    // Only show scene entities at root level
                    let root_scenes: Vec<_> = query_entities(context, LOCAL_TRANSFORM)
                        .into_iter()
                        .filter(|entity| {
                            get_component::<Parent>(context, *entity, PARENT).is_none()
                        })
                        .collect();

                    // Show each scene hierarchy
                    for scene in root_scenes {
                        entity_tree_ui(context, ui, scene);
                    }
                });

                ui.separator();

                // Inspector Section
                if ui
                    .collapsing("Components", |ui| {
                        if let Some(entity) = context.resources.user_interface.selected_entity {
                            entity_inspector_ui(context, ui, entity);
                        } else {
                            ui.vertical_centered(|ui| {
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("No entity selected")
                                        .color(egui::Color32::from_gray(128)),
                                );
                                ui.add_space(8.0);
                            });
                        }
                    })
                    .header_response
                    .clicked()
                {
                    // Optional: handle header click
                }
            });
    });
}

fn top_panel_ui(context: &mut crate::context::Context, ui: &egui::Context) {
    egui::TopBottomPanel::top("menu").show(ui, |ui| {
        egui::menu::bar(ui, |ui| {
            egui::global_theme_preference_switch(ui);
            ui.separator();
            ui.checkbox(
                &mut context.resources.user_interface.show_left_panel,
                "Tree",
            );
            ui.checkbox(
                &mut context.resources.user_interface.show_command_window,
                "Api",
            );
            ui.separator();

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!(
                    "FPS: {:>3}", // Right-align with width of 3
                    context.resources.window.frames_per_second
                ));
            });
        });
    });

    // Update the command window
    if context.resources.user_interface.show_command_window {
        egui::Window::new("Api")
            .resizable(true)
            .default_size([400.0, 300.0])
            .show(ui, |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    ui.set_max_width(380.0);
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Send").clicked() {
                                match &context.resources.user_interface.draft_message {
                                    Message::Command { command } => {
                                        publish_command(context, command.clone());
                                    }
                                    Message::Event { event } => {
                                        publish_event(context, event.clone());
                                    }
                                }
                            }
                            use enum2egui::GuiInspect;
                            context.resources.user_interface.draft_message.ui_mut(ui);
                        });
                    });
                });
            });
    }
}

// Recursively renders the entity tree in the ui system
fn entity_tree_ui(
    context: &mut crate::context::Context,
    ui: &mut egui::Ui,
    entity: crate::context::EntityId,
) {
    use crate::context::*;

    let name = if let Some(Name(name)) = get_component::<Name>(context, entity, NAME) {
        name.to_string()
    } else {
        format!("Entity {}", entity.id)
    };

    let selected = context.resources.user_interface.selected_entity == Some(entity);
    let is_scene = get_component::<Parent>(context, entity, PARENT).is_none();
    let is_camera = get_component::<Camera>(context, entity, CAMERA).is_some();

    let id = ui.make_persistent_id(entity.id);
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                let prefix = if is_scene {
                    "🎬"
                } else if is_camera {
                    "📷"
                } else {
                    "🔵"
                };

                let response = ui.selectable_label(selected, format!("{prefix} {name}"));
                if response.clicked() {
                    context.resources.user_interface.selected_entity = Some(entity);
                    if is_camera {
                        context.resources.active_camera_entity = Some(entity);
                    }
                }

                // Context menu
                response.context_menu(|ui| {
                    // Add "Add Entity" option for scenes (root nodes)
                    if is_scene && ui.button("Add Entity").clicked() {
                        let new_entity =
                            spawn_entities(context, LOCAL_TRANSFORM | GLOBAL_TRANSFORM | NAME, 1)
                                [0];

                        // Set name
                        if let Some(name) = get_component_mut::<Name>(context, new_entity, NAME) {
                            *name = Name(format!("Entity {}", new_entity.id));
                        }

                        // Add parent component and set parent
                        add_components(context, new_entity, PARENT);
                        if let Some(parent) =
                            get_component_mut::<Parent>(context, new_entity, PARENT)
                        {
                            *parent = Parent(entity);
                        }

                        context.resources.user_interface.selected_entity = Some(new_entity);
                        ui.close_menu();
                    }

                    // Add "Reparent to..." submenu
                    ui.menu_button("Reparent to...", |ui| {
                        // Get all potential parent entities (excluding this entity and its descendants)
                        let all_entities = query_entities(context, LOCAL_TRANSFORM);
                        let descendants = query_descendents(context, entity);

                        for potential_parent in all_entities {
                            // Skip if this would create a cycle
                            if descendants.contains(&potential_parent) || potential_parent == entity
                            {
                                continue;
                            }

                            // Get name of potential parent
                            let parent_name = if let Some(Name(name)) =
                                get_component::<Name>(context, potential_parent, NAME)
                            {
                                name.to_string()
                            } else {
                                format!("Entity {}", potential_parent.id)
                            };

                            if ui.button(parent_name).clicked() {
                                // Check for cycles one more time before reparenting
                                if !would_create_cycle(context, entity, potential_parent) {
                                    // Add PARENT component if it doesn't exist
                                    if get_component::<Parent>(context, entity, PARENT).is_none() {
                                        add_components(context, entity, PARENT);
                                    }

                                    // Update parent
                                    if let Some(parent) =
                                        get_component_mut::<Parent>(context, entity, PARENT)
                                    {
                                        *parent = Parent(potential_parent);
                                    }
                                }
                                ui.close_menu();
                            }
                        }

                        // Option to remove parent (make root)
                        if get_component::<Parent>(context, entity, PARENT).is_some() {
                            ui.separator();
                            if ui.button("Make Root (Remove Parent)").clicked() {
                                remove_components(context, entity, PARENT);
                                ui.close_menu();
                            }
                        }
                    });

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
            let children = query_children(context, entity);
            for child in children {
                entity_tree_ui(context, ui, child);
            }
        });
}

fn local_transform_inspector_ui(
    context: &mut crate::context::Context,
    ui: &mut egui::Ui,
    entity: crate::context::EntityId,
) {
    use crate::context::*;
    let mut uniform_scaling = context.resources.user_interface.uniform_scaling;

    ui.group(|ui| {
        ui.label("Transform");
        if let Some(local_transform) =
            get_component_mut::<LocalTransform>(context, entity, LOCAL_TRANSFORM)
        {
            // Translation
            ui.group(|ui| {
                ui.label("Position");
                ui.horizontal(|ui| {
                    ui.label("X");
                    ui.add(egui::DragValue::new(&mut local_transform.translation.x).speed(0.1));
                    ui.label("Y");
                    ui.add(egui::DragValue::new(&mut local_transform.translation.y).speed(0.1));
                    ui.label("Z");
                    ui.add(egui::DragValue::new(&mut local_transform.translation.z).speed(0.1));
                });
            });

            // Scale
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Scale");
                    ui.checkbox(&mut uniform_scaling, "Uniform");
                });

                ui.horizontal(|ui| {
                    ui.label("X");
                    if ui
                        .add(egui::DragValue::new(&mut local_transform.scale.x).speed(0.1))
                        .changed()
                        && uniform_scaling
                    {
                        local_transform.scale.y = local_transform.scale.x;
                        local_transform.scale.z = local_transform.scale.x;
                    }
                    ui.label("Y");
                    if ui
                        .add(egui::DragValue::new(&mut local_transform.scale.y).speed(0.1))
                        .changed()
                        && uniform_scaling
                    {
                        local_transform.scale.x = local_transform.scale.y;
                        local_transform.scale.z = local_transform.scale.y;
                    }
                    ui.label("Z");
                    if ui
                        .add(egui::DragValue::new(&mut local_transform.scale.z).speed(0.1))
                        .changed()
                        && uniform_scaling
                    {
                        local_transform.scale.x = local_transform.scale.z;
                        local_transform.scale.y = local_transform.scale.z;
                    }
                });
            });

            if ui.button("Remove Component").clicked() {
                remove_components(context, entity, LOCAL_TRANSFORM);
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

fn create_scene_pane(context: &mut crate::context::Context) -> Pane {
    // Count only root nodes (no Parent component) for scene numbering
    let scene_count = query_entities(context, LOCAL_TRANSFORM)
        .into_iter()
        .filter(|e| get_component::<Parent>(context, *e, PARENT).is_none())
        .count();

    // Create root entity (scene)
    let scene = spawn_entities(context, NAME | LOCAL_TRANSFORM | GLOBAL_TRANSFORM, 1)[0];

    // Set scene name using root node count
    if let Some(name) = get_component_mut::<Name>(context, scene, NAME) {
        *name = Name(format!("Scene {}", scene_count + 1));
    }

    // Create camera as child
    let camera = spawn_entities(
        context,
        CAMERA | LOCAL_TRANSFORM | GLOBAL_TRANSFORM | NAME | PARENT,
        1,
    )[0];

    // Set camera name to match scene number
    if let Some(name) = get_component_mut::<Name>(context, camera, NAME) {
        *name = Name(format!("Camera {}", scene_count + 1));
    }

    // Set up camera transform
    initialize_camera_transform(context, camera);

    // Parent camera to scene
    if let Some(parent) = get_component_mut::<Parent>(context, camera, PARENT) {
        *parent = Parent(scene);
    }

    // Set as active camera
    context.resources.active_camera_entity = Some(camera);

    Pane {
        kind: PaneKind::Scene {
            scene_entity: scene,
            camera_entity: Some(camera),
        },
    }
}

fn initialize_camera_transform(context: &mut crate::context::Context, camera: EntityId) {
    if let Some(transform) = get_component_mut::<LocalTransform>(context, camera, LOCAL_TRANSFORM) {
        transform.translation = nalgebra_glm::vec3(0.0, 4.0, 5.0);

        let target = nalgebra_glm::Vec3::zeros();
        let up = nalgebra_glm::Vec3::y();

        let forward = nalgebra_glm::normalize(&(target - transform.translation));
        let right = nalgebra_glm::normalize(&nalgebra_glm::cross(&up, &forward));
        let new_up = nalgebra_glm::cross(&forward, &right);

        let rotation_mat = nalgebra_glm::mat3(
            right.x, new_up.x, -forward.x, right.y, new_up.y, -forward.y, right.z, new_up.z,
            -forward.z,
        );
        transform.rotation = nalgebra_glm::mat3_to_quat(&rotation_mat);
    }
}

// Add this helper function to check for cycles
fn would_create_cycle(
    context: &crate::context::Context,
    child: EntityId,
    new_parent: EntityId,
) -> bool {
    // If the new parent is the same as the child, it would create a cycle
    if child == new_parent {
        return true;
    }

    // Check if any ancestor of new_parent is the child
    let mut current = new_parent;
    while let Some(Parent(parent)) = get_component::<Parent>(context, current, PARENT) {
        if *parent == child {
            return true;
        }
        current = *parent;
    }

    false
}

#[derive(Default, Clone)]
pub struct ApiLogEntry {
    pub message: Message,
}

// Update the function to check both text editing and window focus
pub fn should_capture_keyboard(context: &crate::context::Context) -> bool {
    let Some(gui_state) = &context.resources.user_interface.state else {
        return false;
    };

    let ctx = gui_state.egui_ctx();

    // Check if any UI element wants keyboard input OR if any window is focused
    ctx.wants_keyboard_input() || ctx.wants_pointer_input()
}
