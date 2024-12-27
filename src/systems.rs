/// This runs the systems that update the scene
pub fn run_systems(scene: &mut crate::Scene) {
    ensure_tile_tree_system(scene);
}

/// Ensures a default layout when the tile tree is emptied
fn ensure_tile_tree_system(scene: &mut crate::Scene) {
    if scene.resources.tile_tree.is_some() {
        return;
    }
    log::info!("Creating default tile tree");
    let mut tiles = egui_tiles::Tiles::default();
    let mut tab_tiles = vec![];
    let tab_tile_child = tiles.insert_pane(crate::Pane::default());
    let tab_tile = tiles.insert_tab_tile(vec![tab_tile_child]);
    tab_tiles.push(tab_tile);
    let root = tiles.insert_tab_tile(tab_tiles);
    let tiles = egui_tiles::Tree::new("tree", root, tiles);
    scene.resources.tile_tree = Some(tiles);
}
