use serde::{Deserialize, Serialize};

/// Envelope field carrying the serialized message in every `postMessage`.
pub const MESSAGE_KEY: &str = "message";
/// Envelope field carrying the transferred `OffscreenCanvas` (on `Init` only).
pub const CANVAS_KEY: &str = "canvas";
/// Envelope field carrying transferred binary asset bytes (drag-drop).
pub const BYTES_KEY: &str = "bytes";

/// What a dropped binary payload is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetKind {
    /// A glTF or GLB model (loaded with `import_gltf_from_bytes`).
    Model,
    /// An HDR environment map (loaded as the skybox + IBL).
    Hdri,
}

/// Page to worker. Pixel quantities are physical surface pixels (CSS pixels
/// times the device pixel ratio), origin at the canvas top-left.
#[derive(Clone, Serialize, Deserialize)]
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
    /// Orbit deltas in raw pointer pixels.
    Orbit {
        yaw: f32,
        pitch: f32,
    },
    /// Pan deltas in raw pointer pixels.
    Pan {
        dx: f32,
        dy: f32,
    },
    Zoom {
        amount: f32,
    },
    /// Pixel-perfect pick at a click position.
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
    /// Frame the camera on the current selection (or the whole model).
    Frame,
    /// A dropped file's bytes are in the `bytes` field of the envelope.
    DropAsset {
        kind: AssetKind,
    },
    /// Load a Khronos sample model by its index name (worker fetches it).
    LoadKhronos {
        name: String,
    },
    /// Load a Polyhaven HDRI by its slug (worker fetches it).
    LoadPolyhaven {
        slug: String,
    },
    /// Ask the worker to fetch the browser indices if it has not yet.
    RefreshBrowsers,
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
    Ready { context: String, adapter: String },
    Stats { fps: f32 },
    Scene { nodes: Vec<SceneNode> },
    Selected { detail: Option<EntityDetail> },
    Loading { active: bool, label: String },
    KhronosList { entries: Vec<KhronosEntry> },
    PolyhavenList { entries: Vec<PolyhavenEntry> },
}
