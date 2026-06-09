use leptos::prelude::*;
use protocol::{
    ClipInfo, EntityDetail, GamePhase, GizmoKind, KhronosEntry, ModelStats, PbrDebug,
    PolyhavenEntry, SceneNode, ShadingMode, Tonemap,
};

/// Which asset browser overlay, if any, is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Browser {
    Closed,
    Khronos,
    Hdris,
    Models,
}

/// Which tab the left panel shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelTab {
    Scene,
    Render,
    Stats,
}

/// Khronos glTF-Validator result for a dropped model.
#[derive(Clone, Copy)]
pub struct Validation {
    pub errors: u32,
    pub warnings: u32,
}

/// All page state, grouped as signals. `Copy`, so it threads into every
/// component and closure without cloning.
#[derive(Clone, Copy)]
pub struct ViewerState {
    pub context: RwSignal<String>,
    pub adapter: RwSignal<String>,
    pub fps: RwSignal<f32>,
    pub scene: RwSignal<Vec<SceneNode>>,
    pub selected: RwSignal<Option<EntityDetail>>,
    pub loading: RwSignal<Option<String>>,
    pub khronos: RwSignal<Vec<KhronosEntry>>,
    pub hdris: RwSignal<Vec<PolyhavenEntry>>,
    pub models: RwSignal<Vec<PolyhavenEntry>>,
    pub browser: RwSignal<Browser>,
    pub gizmo_mode: RwSignal<GizmoKind>,
    pub camera_basis: RwSignal<[[f32; 3]; 3]>,
    pub resolution: RwSignal<u32>,
    pub grid: RwSignal<bool>,
    pub turntable: RwSignal<bool>,
    pub ui_hidden: RwSignal<bool>,
    pub dragging: RwSignal<bool>,
    pub grabbing: RwSignal<bool>,
    pub ready: RwSignal<bool>,
    pub scene_open: RwSignal<bool>,
    pub inspector_open: RwSignal<bool>,
    pub add_open: RwSignal<bool>,
    pub hint_open: RwSignal<bool>,
    pub tab: RwSignal<PanelTab>,
    pub stats: RwSignal<Option<ModelStats>>,
    pub clips: RwSignal<Vec<ClipInfo>>,
    pub variants: RwSignal<Vec<String>>,
    pub active_variant: RwSignal<Option<String>>,
    pub anim_clip: RwSignal<Option<u32>>,
    pub anim_playing: RwSignal<bool>,
    pub anim_time: RwSignal<f32>,
    pub anim_duration: RwSignal<f32>,
    pub anim_speed: RwSignal<f32>,
    pub anim_loop: RwSignal<bool>,
    pub shading: RwSignal<ShadingMode>,
    pub pbr_debug: RwSignal<PbrDebug>,
    pub show_normals: RwSignal<bool>,
    pub show_bounds: RwSignal<bool>,
    pub show_sky: RwSignal<bool>,
    pub exposure: RwSignal<f32>,
    pub tonemap: RwSignal<Tonemap>,
    pub validation: RwSignal<Option<Validation>>,
    pub game_phase: RwSignal<GamePhase>,
    pub game_level: RwSignal<u32>,
    pub game_score: RwSignal<u32>,
    pub game_shots_left: RwSignal<u32>,
    pub game_shots_total: RwSignal<u32>,
    pub game_targets_left: RwSignal<u32>,
    pub game_targets_total: RwSignal<u32>,
    pub game_combo: RwSignal<u32>,
    pub game_menu_open: RwSignal<bool>,
    pub game_hits: RwSignal<Vec<(u32, String)>>,
}

impl ViewerState {
    pub fn new() -> Self {
        Self {
            context: RwSignal::new("connecting…".to_string()),
            adapter: RwSignal::new(String::new()),
            fps: RwSignal::new(0.0),
            scene: RwSignal::new(Vec::new()),
            selected: RwSignal::new(None),
            loading: RwSignal::new(None),
            khronos: RwSignal::new(Vec::new()),
            hdris: RwSignal::new(Vec::new()),
            models: RwSignal::new(Vec::new()),
            browser: RwSignal::new(Browser::Closed),
            gizmo_mode: RwSignal::new(GizmoKind::Translate),
            camera_basis: RwSignal::new([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
            resolution: RwSignal::new(2),
            grid: RwSignal::new(true),
            turntable: RwSignal::new(false),
            ui_hidden: RwSignal::new(false),
            dragging: RwSignal::new(false),
            grabbing: RwSignal::new(false),
            ready: RwSignal::new(false),
            scene_open: RwSignal::new(false),
            inspector_open: RwSignal::new(false),
            add_open: RwSignal::new(false),
            hint_open: RwSignal::new(!hint_seen()),
            tab: RwSignal::new(PanelTab::Scene),
            stats: RwSignal::new(None),
            clips: RwSignal::new(Vec::new()),
            variants: RwSignal::new(Vec::new()),
            active_variant: RwSignal::new(None),
            anim_clip: RwSignal::new(None),
            anim_playing: RwSignal::new(false),
            anim_time: RwSignal::new(0.0),
            anim_duration: RwSignal::new(0.0),
            anim_speed: RwSignal::new(1.0),
            anim_loop: RwSignal::new(true),
            shading: RwSignal::new(ShadingMode::Rendered),
            pbr_debug: RwSignal::new(PbrDebug::Off),
            show_normals: RwSignal::new(false),
            show_bounds: RwSignal::new(false),
            show_sky: RwSignal::new(true),
            exposure: RwSignal::new(1.0),
            tonemap: RwSignal::new(Tonemap::Aces),
            validation: RwSignal::new(None),
            game_phase: RwSignal::new(GamePhase::Idle),
            game_level: RwSignal::new(1),
            game_score: RwSignal::new(0),
            game_shots_left: RwSignal::new(0),
            game_shots_total: RwSignal::new(0),
            game_targets_left: RwSignal::new(0),
            game_targets_total: RwSignal::new(0),
            game_combo: RwSignal::new(0),
            game_menu_open: RwSignal::new(false),
            game_hits: RwSignal::new(Vec::new()),
        }
    }
}

impl Default for ViewerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Key under which the one-time onboarding hint records that it has been seen.
const HINT_KEY: &str = "nightshade_hint_seen";

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|window| window.local_storage().ok().flatten())
}

/// Whether the onboarding hint has already been dismissed on this browser.
pub fn hint_seen() -> bool {
    local_storage()
        .and_then(|storage| storage.get_item(HINT_KEY).ok().flatten())
        .is_some()
}

/// Records that the onboarding hint has been dismissed so it stays hidden.
pub fn mark_hint_seen() {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(HINT_KEY, "1");
    }
}

/// Key under which the siege game records the highest unlocked level.
const GAME_UNLOCKED_KEY: &str = "nightshade_game_unlocked";

fn game_best_key(level: u32) -> String {
    format!("nightshade_game_best_{level}")
}

fn read_storage_number(key: &str) -> u32 {
    local_storage()
        .and_then(|storage| storage.get_item(key).ok().flatten())
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

/// The highest siege level unlocked on this browser (at least 1).
pub fn game_unlocked() -> u32 {
    read_storage_number(GAME_UNLOCKED_KEY).max(1)
}

/// Unlocks a siege level if it is beyond the current progress.
pub fn unlock_game_level(level: u32) {
    if level > game_unlocked()
        && let Some(storage) = local_storage()
    {
        let _ = storage.set_item(GAME_UNLOCKED_KEY, &level.to_string());
    }
}

/// The best recorded score for a siege level on this browser.
pub fn game_best(level: u32) -> u32 {
    read_storage_number(&game_best_key(level))
}

/// Records a siege level score if it beats the stored best.
pub fn record_game_best(level: u32, score: u32) {
    if score > game_best(level)
        && let Some(storage) = local_storage()
    {
        let _ = storage.set_item(&game_best_key(level), &score.to_string());
    }
}
