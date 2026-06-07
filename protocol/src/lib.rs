use serde::{Deserialize, Serialize};

#[cfg(feature = "agent")]
mod agent;
#[cfg(feature = "agent")]
pub use agent::*;

/// Envelope field carrying the serialized message in every `postMessage`.
pub const MESSAGE_KEY: &str = "message";
/// Envelope field carrying the transferred `OffscreenCanvas` (on `Init` only).
pub const CANVAS_KEY: &str = "canvas";
/// Envelope field carrying transferred binary asset bytes (drag-drop).
pub const BYTES_KEY: &str = "bytes";
/// Envelope field carrying the transferred glTF bytes of a multi-file bundle.
pub const GLTF_KEY: &str = "gltf";
/// Envelope field carrying a `{ name: Uint8Array }` object of bundle resources.
pub const RESOURCES_KEY: &str = "resources";

/// What a dropped binary payload is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "agent", derive(enum2schema::Schema), schema(string_enum))]
pub enum AssetKind {
    /// A glTF or GLB model (loaded with `import_gltf_from_bytes`).
    Model,
    /// An HDR environment map (loaded as the skybox + IBL).
    Hdri,
}

/// Which transform gizmo the manipulation handles show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "agent", derive(enum2schema::Schema), schema(string_enum))]
pub enum GizmoKind {
    Translate,
    Rotate,
    Scale,
}

/// A parametric primitive mesh the viewer can spawn from the Add menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "agent", derive(enum2schema::Schema), schema(string_enum))]
pub enum PrimitiveKind {
    Cube,
    Sphere,
    Cylinder,
    Cone,
    Torus,
    Plane,
}

/// A light the viewer can spawn from the Add menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "agent", derive(enum2schema::Schema), schema(string_enum))]
pub enum LightKind {
    Directional,
    Point,
    Spot,
}

/// Lifecycle phase of a forwarded touch contact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "agent", derive(enum2schema::Schema), schema(string_enum))]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

/// Viewport shading mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "agent", derive(enum2schema::Schema), schema(string_enum))]
pub enum ShadingMode {
    Rendered,
    Solid,
    Flat,
    Wireframe,
}

/// PBR channel debug view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "agent", derive(enum2schema::Schema), schema(string_enum))]
pub enum PbrDebug {
    Off,
    BaseColor,
    Normal,
    Metallic,
    Roughness,
    Occlusion,
    Emissive,
}

/// Tone mapping operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "agent", derive(enum2schema::Schema), schema(string_enum))]
pub enum Tonemap {
    Aces,
    Reinhard,
    Uncharted2,
    AgX,
    Neutral,
    None,
}

/// Counts and size of the loaded model, for the stats panel.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct ModelStats {
    pub meshes: u32,
    pub vertices: u32,
    pub triangles: u32,
    pub materials: u32,
    pub textures: u32,
    pub dimensions: [f32; 3],
}

/// One animation clip's name and length.
#[derive(Clone, Serialize, Deserialize)]
pub struct ClipInfo {
    pub name: String,
    pub duration: f32,
}

/// Page to worker. Pixel quantities are physical surface pixels (CSS pixels
/// times the device pixel ratio), origin at the canvas top-left.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "agent", derive(enum2schema::Schema))]
pub enum ClientMessage {
    /// Sent once with the `OffscreenCanvas` in the transfer list.
    Init {
        width: f32,
        height: f32,
    },
    Resize {
        width: f32,
        height: f32,
    },
    /// Absolute cursor position in physical pixels. Drives the engine camera,
    /// gizmo hover, and gizmo drag.
    PointerMove {
        x: f32,
        y: f32,
    },
    /// A mouse button changed. `button` is 0 left, 1 middle, 2 right.
    PointerButton {
        button: u8,
        pressed: bool,
    },
    /// Wheel delta in raw pixels (the worker converts to scroll lines).
    Wheel {
        delta: f32,
    },
    /// A touch contact in physical pixels. Drives the engine touch controller:
    /// one finger orbits, two fingers pan, a pinch zooms. `id` is the pointer id.
    Touch {
        id: u64,
        phase: TouchPhase,
        x: f32,
        y: f32,
    },
    /// A click without drag: GPU-pick and select (or cycle) at this position.
    Pick {
        x: f32,
        y: f32,
    },
    /// Select an entity by raw id (from the scene tree).
    Select {
        id: u32,
    },
    /// Clear the current selection.
    Deselect,
    /// Edit the selected entity's local transform. Rotation is Euler degrees.
    SetTransform {
        id: u32,
        translation: [f32; 3],
        rotation: [f32; 3],
        scale: [f32; 3],
    },
    /// Choose which transform gizmo the handles show.
    SetGizmoMode {
        mode: GizmoKind,
    },
    /// Snap the camera to look along a world axis (the clicked nav-gizmo axis).
    SnapAxis {
        axis: [f32; 3],
    },
    /// Play an animation clip by index.
    PlayAnimation {
        index: u32,
    },
    /// Pause the current animation.
    PauseAnimation,
    /// Resume the current animation.
    ResumeAnimation,
    /// Stop and rewind the animation.
    StopAnimation,
    /// Scrub the animation to a time in seconds.
    SeekAnimation {
        time: f32,
    },
    /// Set animation playback speed.
    SetAnimationSpeed {
        speed: f32,
    },
    /// Toggle animation looping.
    SetAnimationLoop {
        looping: bool,
    },
    /// Set the viewport shading mode.
    SetShadingMode {
        mode: ShadingMode,
    },
    /// Set the PBR channel debug view.
    SetPbrDebug {
        mode: PbrDebug,
    },
    /// Toggle the normal-line overlay.
    SetShowNormals {
        enabled: bool,
    },
    /// Toggle the bounding-volume overlay.
    SetShowBounds {
        enabled: bool,
    },
    /// Set the exposure multiplier.
    SetExposure {
        exposure: f32,
    },
    /// Set the tone mapping operator.
    SetTonemap {
        algorithm: Tonemap,
    },
    /// Toggle the visible skybox (IBL stays on).
    SetShowSky {
        show: bool,
    },
    /// Apply a material variant by name, or `None` to reset to default.
    SetVariant {
        name: Option<String>,
    },
    /// Frame the camera on the current selection (or the whole model).
    Frame,
    /// Toggle the auto-rotating turntable camera.
    SetTurntable {
        enabled: bool,
    },
    /// A dropped file's bytes are in the `bytes` field of the envelope.
    DropAsset {
        kind: AssetKind,
    },
    /// A multi-file glTF: the `gltf` field holds the document bytes and the
    /// `resources` field a `{ name: Uint8Array }` object of buffers and images.
    LoadGltfBundle,
    /// Load a Khronos sample model by its index name (worker fetches it).
    LoadKhronos {
        name: String,
    },
    /// Load a Polyhaven HDRI by its slug at the given resolution in k (1, 2, 4…).
    LoadPolyhaven {
        slug: String,
        resolution: u32,
    },
    /// Load a Polyhaven model by its slug at the given texture resolution in k.
    LoadPolyhavenModel {
        slug: String,
        resolution: u32,
    },
    /// Toggle the world ground grid.
    SetGrid {
        enabled: bool,
    },
    /// Spawn a parametric primitive mesh at the camera focus and select it.
    AddPrimitive {
        kind: PrimitiveKind,
    },
    /// Spawn a light (with a small emissive marker) near the camera focus and
    /// select it.
    AddLight {
        kind: LightKind,
    },
    /// Ask the worker to fetch the browser indices if it has not yet.
    RefreshBrowsers,
    /// External agent traffic (Claude Code via the MCP host), forwarded over the
    /// page's WebSocket relay onto this same postMessage path.
    #[cfg(feature = "agent")]
    #[schema(skip)]
    Agent(AgentRequest),
}

/// One row of the flattened scene tree.
#[derive(Clone, Serialize, Deserialize)]
pub struct SceneNode {
    pub id: u32,
    pub name: String,
    pub depth: u32,
    pub has_mesh: bool,
}

/// The selected entity's editable detail. Rotation is Euler degrees.
#[derive(Clone, Serialize, Deserialize)]
pub struct EntityDetail {
    pub id: u32,
    pub name: String,
    pub translation: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    pub mesh: Option<String>,
}

/// A Khronos sample-asset entry for the browser grid.
#[derive(Clone, Serialize, Deserialize)]
pub struct KhronosEntry {
    pub name: String,
    pub label: String,
    pub thumbnail: Option<String>,
}

/// A Polyhaven HDRI entry for the browser grid.
#[derive(Clone, Serialize, Deserialize)]
pub struct PolyhavenEntry {
    pub slug: String,
    pub name: String,
    pub thumbnail: String,
}

/// Worker to page.
#[derive(Clone, Serialize, Deserialize)]
pub enum WorkerMessage {
    Ready {
        context: String,
        adapter: String,
    },
    Stats {
        fps: f32,
    },
    /// The active camera's world-space basis, for the page's nav gizmo.
    Camera {
        right: [f32; 3],
        up: [f32; 3],
        forward: [f32; 3],
    },
    /// Model facts sent once per load.
    Loaded {
        stats: ModelStats,
        clips: Vec<ClipInfo>,
        variants: Vec<String>,
        exposure: f32,
    },
    /// Animation playhead, streamed while a clip plays.
    Animation {
        time: f32,
        duration: f32,
        playing: bool,
        clip: Option<u32>,
    },
    Scene {
        nodes: Vec<SceneNode>,
    },
    Selected {
        detail: Option<EntityDetail>,
    },
    Loading {
        active: bool,
        label: String,
    },
    KhronosList {
        entries: Vec<KhronosEntry>,
    },
    PolyhavenList {
        entries: Vec<PolyhavenEntry>,
    },
    PolyhavenModelsList {
        entries: Vec<PolyhavenEntry>,
    },
    /// External agent responses and delta batches, relayed back to the MCP host.
    #[cfg(feature = "agent")]
    Agent(AgentResponse),
}
