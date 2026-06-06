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

/// Which transform gizmo the manipulation handles show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GizmoKind {
    Translate,
    Rotate,
    Scale,
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
    PolyhavenModelsList { entries: Vec<PolyhavenEntry> },
}
