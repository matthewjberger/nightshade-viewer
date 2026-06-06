use leptos::prelude::*;
use protocol::{
    ClipInfo, EntityDetail, GizmoKind, KhronosEntry, ModelStats, PbrDebug, PolyhavenEntry,
    SceneNode, ShadingMode, Tonemap,
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
    pub dragging: RwSignal<bool>,
    pub grabbing: RwSignal<bool>,
    pub ready: RwSignal<bool>,
    pub scene_open: RwSignal<bool>,
    pub inspector_open: RwSignal<bool>,
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
            dragging: RwSignal::new(false),
            grabbing: RwSignal::new(false),
            ready: RwSignal::new(false),
            scene_open: RwSignal::new(false),
            inspector_open: RwSignal::new(false),
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
        }
    }
}

impl Default for ViewerState {
    fn default() -> Self {
        Self::new()
    }
}
