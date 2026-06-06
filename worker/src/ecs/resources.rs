use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use nightshade::prelude::Entity;

/// Camera requests from the page. Orbit, pan, and zoom now come from forwarded
/// pointer input the engine reads directly, so only framing remains here.
#[derive(Default)]
pub struct CameraInput {
    pub frame_requested: bool,
}

/// The currently selected engine entity.
#[derive(Default)]
pub struct Selection {
    pub selected: Option<Entity>,
}

/// The loaded model: the spawned prefab roots and every entity under them.
#[derive(Default)]
pub struct Model {
    pub roots: Vec<Entity>,
    pub entities: Vec<Entity>,
}

/// Pending GPU pick plus the click-cycle state (repeated clicks on one spot
/// walk from the model root down to the leaf, like the editor).
#[derive(Default)]
pub struct Picking {
    pub pending: bool,
    pub last_leaf: Option<Entity>,
    pub last_root: Option<Entity>,
    pub cycle_depth: usize,
}

/// Set when the scene tree or selection should be re-sent to the page.
#[derive(Default)]
pub struct SceneSync {
    pub needs_tree: bool,
    pub needs_selection: bool,
}

/// A binary asset waiting to be loaded into the engine.
pub enum PendingAsset {
    Model(Vec<u8>),
    ModelWithResources {
        gltf: Vec<u8>,
        resources: HashMap<String, Vec<u8>>,
    },
    Hdri(Vec<u8>),
}

/// Inbox for asset bytes from drops or browser fetches, plus the loading label
/// that drives the page's progress indicator. The `Arc<Mutex<…>>` lets `ehttp`
/// callbacks (which require `Send + 'static`) write results back.
pub struct Incoming {
    pub asset: Arc<Mutex<Option<PendingAsset>>>,
    pub loading: Arc<Mutex<Option<String>>>,
}

impl Default for Incoming {
    fn default() -> Self {
        Self {
            asset: Arc::new(Mutex::new(None)),
            loading: Arc::new(Mutex::new(None)),
        }
    }
}

/// A Khronos sample-asset index entry.
#[derive(Clone)]
pub struct KhronosAsset {
    pub name: String,
    pub label: String,
    pub glb_url: Option<String>,
    pub thumbnail: Option<String>,
}

/// A Polyhaven index entry (HDRI or model).
#[derive(Clone)]
pub struct PolyAsset {
    pub slug: String,
    pub name: String,
    pub thumbnail: String,
}

/// Async fetch state for a browser index.
#[derive(Default)]
pub enum FetchState<T> {
    #[default]
    Idle,
    Loading,
    Loaded(T),
    Failed,
}

/// Browser index fetch state. The lists are streamed to the page once loaded.
pub struct Browsers {
    pub khronos: Arc<Mutex<FetchState<Vec<KhronosAsset>>>>,
    pub hdris: Arc<Mutex<FetchState<Vec<PolyAsset>>>>,
    pub models: Arc<Mutex<FetchState<Vec<PolyAsset>>>>,
    pub khronos_sent: bool,
    pub hdris_sent: bool,
    pub models_sent: bool,
}

impl Default for Browsers {
    fn default() -> Self {
        Self {
            khronos: Arc::new(Mutex::new(FetchState::Idle)),
            hdris: Arc::new(Mutex::new(FetchState::Idle)),
            models: Arc::new(Mutex::new(FetchState::Idle)),
            khronos_sent: false,
            hdris_sent: false,
            models_sent: false,
        }
    }
}
