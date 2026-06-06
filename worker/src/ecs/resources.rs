use std::sync::{Arc, Mutex};

use nightshade::prelude::Entity;
use protocol::AssetKind;

/// Per-frame camera deltas forwarded from the page, applied then cleared.
#[derive(Default)]
pub struct CameraInput {
    pub pending_yaw: f32,
    pub pending_pitch: f32,
    pub pending_pan_x: f32,
    pub pending_pan_y: f32,
    pub pending_zoom: f32,
    pub frame_requested: bool,
}

/// The currently selected engine entity.
#[derive(Default)]
pub struct Selection {
    pub selected: Option<Entity>,
}

/// The loaded model: its root and every spawned entity (for despawn + tree).
#[derive(Default)]
pub struct Model {
    pub root: Option<Entity>,
    pub entities: Vec<Entity>,
}

/// Pending GPU pick request.
#[derive(Default)]
pub struct Picking {
    pub pending: bool,
}

/// Set when the scene tree or selection should be re-sent to the page.
#[derive(Default)]
pub struct SceneSync {
    pub needs_tree: bool,
    pub needs_selection: bool,
}

/// A binary asset waiting to be loaded into the engine.
pub struct PendingAsset {
    pub kind: AssetKind,
    pub bytes: Vec<u8>,
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

/// A Polyhaven HDRI index entry.
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
    pub polyhaven: Arc<Mutex<FetchState<Vec<PolyAsset>>>>,
    pub khronos_sent: bool,
    pub polyhaven_sent: bool,
}

impl Default for Browsers {
    fn default() -> Self {
        Self {
            khronos: Arc::new(Mutex::new(FetchState::Idle)),
            polyhaven: Arc::new(Mutex::new(FetchState::Idle)),
            khronos_sent: false,
            polyhaven_sent: false,
        }
    }
}
