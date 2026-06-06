use leptos::prelude::*;
use protocol::{EntityDetail, KhronosEntry, PolyhavenEntry, SceneNode};

/// Which asset browser overlay, if any, is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Browser {
    Closed,
    Khronos,
    Polyhaven,
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
    pub polyhaven: RwSignal<Vec<PolyhavenEntry>>,
    pub browser: RwSignal<Browser>,
    pub dragging: RwSignal<bool>,
    pub grabbing: RwSignal<bool>,
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
            polyhaven: RwSignal::new(Vec::new()),
            browser: RwSignal::new(Browser::Closed),
            dragging: RwSignal::new(false),
            grabbing: RwSignal::new(false),
        }
    }
}

impl Default for ViewerState {
    fn default() -> Self {
        Self::new()
    }
}
