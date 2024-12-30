/// This is the main loop, driven by winit window events.
/// Resources are updated and then systems are triggered continuously.
pub fn step(scene: &mut crate::Scene, event: &winit::event::WindowEvent) {
    // On wasm, the renderer is received from an async task
    // and will not be available in the cycles prior to that
    #[cfg(target_arch = "wasm32")]
    crate::systems::receive_renderer_system(scene);

    // The renderer should be available before running any systems
    if scene.resources.graphics.renderer.is_none() {
        return;
    }

    receive_ui_events(scene, event);
    receive_resize_event(scene, event);
    receive_keyboard_event(scene, event);
    receive_mouse_event(scene, event);

    let winit::event::WindowEvent::RedrawRequested = event else {
        return;
    };
    run_systems(scene);
    reset_systems(scene);
}

/// Run systems meant to update each cycle of the main loop
fn run_systems(scene: &mut crate::Scene) {
    update_frame_timing_system(scene);
    ensure_tile_tree_system(scene);
    ui_system(scene);
    render_system(scene);
}

/// Reset systems in preparation the next frame
fn reset_systems(scene: &mut crate::Scene) {
    reset_mouse_system(scene);
}

/// Winit window events drive the main loop,
/// and this module contains stateless free functions that run
/// systems in response to those events
use events::*;
pub mod events {
    pub fn receive_ui_events(scene: &mut crate::Scene, event: &winit::event::WindowEvent) {
        let Some(gui_state) = &mut scene.resources.user_interface.state else {
            return;
        };
        let Some(window_handle) = scene.resources.window.handle.as_ref() else {
            return;
        };
        scene.resources.user_interface.consumed_event =
            gui_state.on_window_event(window_handle, event).consumed;
    }

    pub fn receive_resize_event(scene: &mut crate::Scene, event: &winit::event::WindowEvent) {
        let winit::event::WindowEvent::Resized(winit::dpi::PhysicalSize { width, height }) = event
        else {
            return;
        };
        super::resize_viewport(scene, *width, *height);
    }

    pub fn receive_keyboard_event(scene: &mut crate::Scene, event: &winit::event::WindowEvent) {
        let winit::event::WindowEvent::KeyboardInput {
            event:
                winit::event::KeyEvent {
                    physical_key: winit::keyboard::PhysicalKey::Code(key_code),
                    state,
                    ..
                },
            ..
        } = event
        else {
            return;
        };
        *scene
            .resources
            .input
            .keyboard
            .keystates
            .entry(*key_code)
            .or_insert(*state) = *state;
    }

    pub fn receive_mouse_event(scene: &mut crate::Scene, event: &winit::event::WindowEvent) {
        let mouse = &mut scene.resources.input.mouse;
        match event {
            winit::event::WindowEvent::MouseInput { button, state, .. } => {
                let clicked = *state == winit::event::ElementState::Pressed;
                match button {
                    winit::event::MouseButton::Left => {
                        mouse
                            .buttons
                            .set(crate::MouseButtons::LEFT_CLICKED, clicked);
                    }
                    winit::event::MouseButton::Middle => {
                        mouse
                            .buttons
                            .set(crate::MouseButtons::MIDDLE_CLICKED, clicked);
                    }
                    winit::event::MouseButton::Right => {
                        mouse
                            .buttons
                            .set(crate::MouseButtons::RIGHT_CLICKED, clicked);
                    }
                    _ => {}
                }
            }
            winit::event::WindowEvent::CursorMoved { position, .. } => {
                let last_position = mouse.position;
                let current_position = nalgebra_glm::vec2(position.x as _, position.y as _);
                mouse.position = current_position;
                mouse.position_delta = current_position - last_position;
                mouse.buttons.set(crate::MouseButtons::MOVED, true);
            }
            winit::event::WindowEvent::MouseWheel {
                delta: winit::event::MouseScrollDelta::LineDelta(h_lines, v_lines),
                ..
            } => {
                mouse.wheel_delta = nalgebra_glm::vec2(*h_lines, *v_lines);
                mouse.buttons.set(crate::MouseButtons::SCROLLED, true);
            }
            _ => {}
        }
    }
}

/// This module contains a majority of the business logic of the application.
///
/// Systems are stateless free functions that operate on the scene data.
/// State associated with them goes into the world resources.
/// They may also queries and commands to interact with the scene data.
use systems::*;
pub mod systems {
    /// Calculates and refreshes frame timing values such as delta time
    pub fn update_frame_timing_system(scene: &mut crate::Scene) {
        let now = web_time::Instant::now();

        let crate::Scene {
            resources:
                crate::Resources {
                    frame_timing:
                        crate::FrameTiming {
                            frames_per_second,
                            delta_time,
                            last_frame_start_instant,
                            current_frame_start_instant,
                            initial_frame_start_instant,
                            frame_counter,
                            uptime_milliseconds,
                        },
                    ..
                },
            ..
        } = scene;

        // Capture first instant
        if initial_frame_start_instant.is_none() {
            *initial_frame_start_instant = Some(now);
        }

        // Delta time
        *delta_time =
            last_frame_start_instant.map_or(0.0, |last_frame| (now - last_frame).as_secs_f32());

        // Last frame start
        *last_frame_start_instant = Some(now);

        // Current frame start
        if current_frame_start_instant.is_none() {
            *current_frame_start_instant = Some(now);
        }

        // Calculate uptime
        if let Some(app_start) = *initial_frame_start_instant {
            *uptime_milliseconds = (now - app_start).as_millis() as u64;
        }

        // Calculate frames per second
        *frame_counter += 1;
        match current_frame_start_instant.as_ref() {
            Some(start) => {
                if (now - *start).as_secs_f32() >= 1.0 {
                    *frames_per_second = *frame_counter as f32;
                    *frame_counter = 0;
                    *current_frame_start_instant = Some(now);
                }
            }
            None => {
                *current_frame_start_instant = Some(now);
            }
        }
    }

    /// Ensures a default layout when the tile tree is emptied
    pub fn ensure_tile_tree_system(scene: &mut crate::Scene) {
        if let Some(tile_tree) = &scene.resources.user_interface.tile_tree {
            if !tile_tree.tiles.is_empty() {
                return;
            }
        }
        let mut tiles = egui_tiles::Tiles::default();
        let mut tab_tiles = vec![];
        let tab_tile_child = tiles.insert_pane(crate::Pane::default());
        let tab_tile = tiles.insert_tab_tile(vec![tab_tile_child]);
        tab_tiles.push(tab_tile);
        let root = tiles.insert_tab_tile(tab_tiles);
        let tiles = egui_tiles::Tree::new("tree", root, tiles);
        scene.resources.user_interface.tile_tree = Some(tiles);
    }

    /// Creates the UI for the frame and
    /// emits the resources needed for rendering
    pub fn ui_system(scene: &mut crate::Scene) {
        let ui = {
            let Some(gui_state) = scene.resources.user_interface.state.as_mut() else {
                return;
            };
            let Some(window_handle) = scene.resources.window.handle.as_ref() else {
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
                    scene.resources.frame_timing.frames_per_second
                ));
                ui.separator();
                ui.checkbox(
                    &mut scene.resources.user_interface.show_left_panel,
                    "Show Left Panel",
                );
                ui.checkbox(
                    &mut scene.resources.user_interface.show_right_panel,
                    "Show Right Panel",
                );
                ui.separator();
            });
        });

        if scene.resources.user_interface.show_left_panel {
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
                                            let entity =
                                                crate::spawn_entities(scene, crate::VISIBLE, 1)[0];
                                            scene.resources.user_interface.selected_entity =
                                                Some(entity);
                                        }
                                        crate::query_root_nodes(scene).into_iter().for_each(
                                            |entity| {
                                                entity_tree_ui(scene, ui, entity);
                                            },
                                        );
                                    });
                                });
                        });
                        ui.separator();
                    });
            });
        }

        if scene.resources.user_interface.show_right_panel {
            egui::SidePanel::right("right").show(&ui, |ui| {
                ui.collapsing("Properties", |_ui| {
                    //
                });
            });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(&ui, |ui| {
                let crate::UserInterface {
                    tile_tree: Some(tile_tree),
                    tile_tree_behavior: tile_tree_context,
                    ..
                } = &mut scene.resources.user_interface
                else {
                    return;
                };
                tile_tree.ui(tile_tree_context, ui);
                if let Some(parent) = tile_tree_context.add_child_to.take() {
                    let new_child = tile_tree.tiles.insert_pane(crate::Pane {});
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
        scene.resources.user_interface.frame_output = Some((output, paint_jobs));
    }

    /// Resets the mouse state for the next frame
    pub fn reset_mouse_system(scene: &mut crate::Scene) {
        let mouse = &mut scene.resources.input.mouse;
        if mouse.buttons.contains(crate::MouseButtons::SCROLLED) {
            mouse.wheel_delta = nalgebra_glm::vec2(0.0, 0.0);
        }
        mouse.buttons.set(crate::MouseButtons::MOVED, false);
        if !mouse.buttons.contains(crate::MouseButtons::MOVED) {
            mouse.position_delta = nalgebra_glm::vec2(0.0, 0.0);
        }
        mouse.buttons.set(crate::MouseButtons::MOVED, false);
    }

    // Recursively renders the entity tree in the ui system
    pub fn entity_tree_ui(scene: &mut crate::Scene, ui: &mut egui::Ui, entity: crate::EntityId) {
        let name = match crate::get_component::<crate::Name>(scene, entity, crate::NAME) {
            Some(crate::Name(name)) if !name.is_empty() => name.to_string(),
            _ => "Entity".to_string(),
        };

        let selected = scene.resources.user_interface.selected_entity == Some(entity);

        let id = ui.make_persistent_id(ui.next_auto_id());
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
            .show_header(ui, |ui| {
                ui.horizontal(|ui| {
                    let prefix = "🔵".to_string();
                    let response = ui.selectable_label(selected, format!("{prefix}{name}"));

                    if response.clicked() {
                        scene.resources.user_interface.selected_entity = Some(entity);
                    }

                    response.context_menu(|ui| {
                        if ui.button("Add Child").clicked() {
                            let child =
                                crate::spawn_entities(scene, crate::PARENT | crate::VISIBLE, 1)[0];
                            if let Some(parent) = crate::get_component_mut::<crate::Parent>(
                                scene,
                                child,
                                crate::PARENT,
                            ) {
                                *parent = crate::Parent(entity);
                            }
                            ui.close_menu();
                        }
                        if ui.button("Remove").clicked() {
                            crate::despawn_entities(scene, &[entity]);
                            let descendents = crate::query_descendents(scene, entity);
                            for entity in descendents {
                                crate::despawn_entities(scene, &[entity]);
                            }
                            ui.close_menu();
                        }
                    });
                });
            })
            .body(|ui| {
                crate::query_children(scene, entity)
                    .into_iter()
                    .for_each(|child| {
                        entity_tree_ui(scene, ui, child);
                    });
            });
    }

    /// Renders graphics to the window
    pub fn render_system(scene: &mut crate::Scene) {
        let Some((egui::FullOutput { textures_delta, .. }, paint_jobs)) =
            scene.resources.user_interface.frame_output.take()
        else {
            return;
        };
        let Some(window_handle) = scene.resources.window.handle.as_ref() else {
            return;
        };
        let screen_descriptor = {
            let (width, height) = scene.resources.graphics.viewport_size;
            egui_wgpu::ScreenDescriptor {
                size_in_pixels: [width, height],
                pixels_per_point: window_handle.scale_factor() as f32,
            }
        };
        let delta_time = scene.resources.frame_timing.delta_time;
        if let Some(renderer) = scene.resources.graphics.renderer.as_mut() {
            renderer.render_frame(screen_descriptor, paint_jobs, textures_delta, delta_time);
        }
    }

    /// Receives the renderer from the async task that creates it on wasm, injecting it as a resource
    #[cfg(target_arch = "wasm32")]
    pub fn receive_renderer_system(scene: &mut crate::Scene) {
        if let Some(receiver) = scene.resources.graphics.renderer_receiver.as_mut() {
            if let Ok(Some(renderer)) = receiver.try_recv() {
                scene.resources.graphics.renderer = Some(renderer);
                scene.resources.graphics.renderer_receiver = None;
            }
        }
        if scene.resources.graphics.renderer.is_none() {
            return;
        }
    }
}

/// Commands are operations that mutate the scene data.
/// They may require arguments and are intended to be used by systems to reuse mutation logic.
use commands::*;
pub mod commands {
    /// Initializes scene resources on startup
    pub fn initialize(scene: &mut crate::Scene) {
        let window_handle = {
            let Some(window_handle) = scene.resources.window.handle.as_mut() else {
                return;
            };
            window_handle.clone()
        };

        #[cfg(not(target_arch = "wasm32"))]
        {
            let inner_size = window_handle.inner_size();
            scene.resources.graphics.viewport_size = (inner_size.width, inner_size.height);
        }

        let gui_context = egui::Context::default();

        let viewport_id = gui_context.viewport_id();
        let gui_state = egui_winit::State::new(
            gui_context,
            viewport_id,
            &window_handle,
            Some(window_handle.scale_factor() as _),
            Some(winit::window::Theme::Dark),
            None,
        );

        #[cfg(not(target_arch = "wasm32"))]
        let (width, height) = (
            window_handle.inner_size().width,
            window_handle.inner_size().height,
        );

        #[cfg(not(target_arch = "wasm32"))]
        {
            env_logger::init();
            let renderer = pollster::block_on(async move {
                crate::graphics::Renderer::new(window_handle.clone(), width, height).await
            });
            scene.resources.graphics.renderer = Some(renderer);
        }

        #[cfg(target_arch = "wasm32")]
        {
            let (sender, receiver) = futures::channel::oneshot::channel();
            scene.resources.graphics.renderer_receiver = Some(receiver);
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init().expect("Failed to initialize logger!");
            let (canvas_width, canvas_height) = scene.resources.graphics.viewport_size;
            log::info!("Canvas dimensions: ({canvas_width} x {canvas_height})");
            wasm_bindgen_futures::spawn_local(async move {
                let renderer = crate::graphics::Renderer::new(
                    window_handle.clone(),
                    canvas_width,
                    canvas_height,
                )
                .await;
                if sender.send(renderer).is_err() {
                    log::error!("Failed to create and send renderer!");
                }
            });
        }

        scene.resources.user_interface.state = Some(gui_state);
        scene.resources.frame_timing.last_frame_start_instant = Some(web_time::Instant::now());
    }

    /// Handles viewport resizing, such as when the window is resized by the user
    pub fn resize_viewport(scene: &mut crate::Scene, width: u32, height: u32) {
        log::info!("Resizing renderer surface to: ({width}, {height})");
        if let Some(renderer) = scene.resources.graphics.renderer.as_mut() {
            renderer.resize(width, height);
        }
        scene.resources.graphics.viewport_size = (width, height);

        // Update the egui context with the new scale factor
        if let (Some(window_handle), Some(gui_state)) = (
            scene.resources.window.handle.as_ref(),
            scene.resources.user_interface.state.as_mut(),
        ) {
            gui_state
                .egui_ctx()
                .set_pixels_per_point(window_handle.scale_factor() as f32);
        }
    }
}

/// Queries are read-only operations
/// that use the scene data to extract information.
/// They are useful for finding entities and components,
/// like the first available camera in a scene.
/// They intentionally do not mutate the scene.
pub use queries::*;
pub mod queries {
    /// Queries for the root nodes of the scene
    /// by looking for entities that do not have a Parent component
    pub fn query_root_nodes(scene: &crate::Scene) -> Vec<crate::EntityId> {
        let mut root_entities: Vec<crate::EntityId> = scene
            .tables
            .iter()
            .filter_map(|table| {
                if crate::has_components!(table, crate::PARENT) {
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
        scene: &crate::Scene,
        target_entity: crate::EntityId,
    ) -> Vec<crate::EntityId> {
        let mut child_entities = Vec::new();
        crate::query_entities(scene, crate::PARENT)
            .into_iter()
            .for_each(|entity| {
                if let Some(crate::Parent(parent_entity)) =
                    crate::get_component(scene, entity, crate::PARENT)
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
        scene: &crate::Scene,
        target_entity: crate::EntityId,
    ) -> Vec<crate::EntityId> {
        let mut descendents = Vec::new();
        let mut stack = vec![target_entity];
        while let Some(entity) = stack.pop() {
            descendents.push(entity);
            query_children(scene, entity).into_iter().for_each(|child| {
                stack.push(child);
            });
        }
        descendents
    }
}
