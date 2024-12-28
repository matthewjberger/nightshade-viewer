/// This runs the systems that update the scene
pub fn run_systems(scene: &mut crate::Scene) {
    update_frame_timing_system(scene);
    ensure_tile_tree_system(scene);
}

/// Calculates and refreshes frame timing values such as delta time
fn update_frame_timing_system(scene: &mut crate::Scene) {
    let now = crate::Instant::now();

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
fn ensure_tile_tree_system(scene: &mut crate::Scene) {
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
pub fn ui_system(
    scene: &mut crate::Scene,
) -> Option<(
    egui::Context,
    egui::TexturesDelta,
    Vec<egui::epaint::ClippedShape>,
    f32,
)> {
    let ui = {
        let Some(gui_state) = scene.resources.user_interface.gui_state.as_mut() else {
            return None;
        };
        let Some(window_handle) = scene.resources.window.handle.as_ref() else {
            return None;
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
    let crate::UserInterface {
        tile_tree: Some(tile_tree),
        tile_tree_context,
        gui_state: Some(gui_state),
        show_left_panel,
        show_right_panel,
    } = &mut scene.resources.user_interface
    else {
        return None;
    };
    if *show_left_panel {
        egui::SidePanel::left("left").show(gui_state.egui_ctx(), |ui| {
            ui.collapsing("Scene", |_ui| {});
        });
    }
    if *show_right_panel {
        egui::SidePanel::right("right").show(gui_state.egui_ctx(), |ui| {
            ui.heading("Inspector");
        });
    }
    egui::CentralPanel::default()
        .frame(egui::Frame::none())
        .show(&ui, |ui| {
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
    let egui_winit::egui::FullOutput {
        textures_delta,
        shapes,
        pixels_per_point,
        ..
    } = ui.end_pass();
    Some((ui, textures_delta, shapes, pixels_per_point))
}
