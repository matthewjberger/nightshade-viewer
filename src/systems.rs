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
    if let Some(tile_tree) = &scene.resources.tile_tree {
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
    scene.resources.tile_tree = Some(tiles);
}
