use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use nightshade::prelude::Entity;
use protocol::{ClipInfo, GamePhase, ModelStats};

/// Camera requests from the page. Orbit, pan, and zoom now come from forwarded
/// pointer input the engine reads directly, so only framing and the turntable
/// toggle remain here.
#[derive(Default)]
pub struct CameraInput {
    pub frame_requested: bool,
    pub turntable: bool,
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
    pub report: Option<LoadReport>,
}

/// A pending load report, sent to the page after a frame so the world-space
/// dimensions are computed from updated global transforms.
pub struct LoadReport {
    pub stats: ModelStats,
    pub clips: Vec<ClipInfo>,
    pub variants: Vec<String>,
    pub exposure: f32,
    pub delay: u32,
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

/// The siege game: arena entities, scoreboard, and the timers that drive
/// combo decay, projectile cleanup, and the failed-level settle check.
pub struct Game {
    pub phase: GamePhase,
    pub level: u32,
    pub score: u32,
    pub shots_left: u32,
    pub shots_total: u32,
    pub targets_total: u32,
    pub arena: Vec<Entity>,
    pub blocks: Vec<Entity>,
    pub targets: Vec<(Entity, f32)>,
    pub projectiles: Vec<(Entity, f32)>,
    pub combo: u32,
    pub combo_timer: f32,
    pub settle_timer: f32,
    pub grid_was_enabled: bool,
    pub dirty: bool,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            phase: GamePhase::Idle,
            level: 1,
            score: 0,
            shots_left: 0,
            shots_total: 0,
            targets_total: 0,
            arena: Vec::new(),
            blocks: Vec::new(),
            targets: Vec::new(),
            projectiles: Vec::new(),
            combo: 0,
            combo_timer: 0.0,
            settle_timer: 0.0,
            grid_was_enabled: true,
            dirty: false,
        }
    }
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

/// A queue of fetched agent assets, each paired with the command correlation id
/// to acknowledge once it has spawned.
#[cfg(feature = "agent")]
pub type AgentLoadQueue = Arc<Mutex<Vec<(u64, Vec<u8>)>>>;

/// A fetched multi-file model (glTF plus its texture/buffer resources) waiting to
/// spawn additively, with the command correlation id to acknowledge.
#[cfg(feature = "agent")]
pub struct AgentModelLoad {
    pub correlation_id: u64,
    pub gltf: Vec<u8>,
    pub resources: HashMap<String, Vec<u8>>,
}

/// A queue of fetched multi-file agent models.
#[cfg(feature = "agent")]
pub type AgentModelQueue = Arc<Mutex<Vec<AgentModelLoad>>>;

/// Inbox for asset bytes from drops or browser fetches, plus the loading label
/// that drives the page's progress indicator. The `Arc<Mutex<…>>` lets `ehttp`
/// callbacks (which require `Send + 'static`) write results back.
///
/// `agent_loads` is a separate queue for the external agent's `load_gltf`: those
/// spawn additively (they do not wipe the current scene), and each carries the
/// correlation id to acknowledge once the model has spawned.
pub struct Incoming {
    pub asset: Arc<Mutex<Option<PendingAsset>>>,
    pub loading: Arc<Mutex<Option<String>>>,
    #[cfg(feature = "agent")]
    pub agent_loads: AgentLoadQueue,
    #[cfg(feature = "agent")]
    pub agent_hdris: AgentLoadQueue,
    #[cfg(feature = "agent")]
    pub agent_models: AgentModelQueue,
}

impl Default for Incoming {
    fn default() -> Self {
        Self {
            asset: Arc::new(Mutex::new(None)),
            loading: Arc::new(Mutex::new(None)),
            #[cfg(feature = "agent")]
            agent_loads: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "agent")]
            agent_hdris: Arc::new(Mutex::new(Vec::new())),
            #[cfg(feature = "agent")]
            agent_models: Arc::new(Mutex::new(Vec::new())),
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
    #[cfg(feature = "agent")]
    pub categories: Vec<String>,
    #[cfg(feature = "agent")]
    pub tags: Vec<String>,
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
