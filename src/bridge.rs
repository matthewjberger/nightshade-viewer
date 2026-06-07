#[cfg(feature = "agent")]
use std::cell::RefCell;
#[cfg(feature = "agent")]
use std::rc::Rc;

use leptos::prelude::*;
use protocol::{
    AssetKind, BYTES_KEY, CANVAS_KEY, ClientMessage, GLTF_KEY, MESSAGE_KEY, RESOURCES_KEY,
    WorkerMessage,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    Blob, DataTransfer, File, MessageEvent, OffscreenCanvas, Worker, WorkerOptions, WorkerType,
};

#[cfg(feature = "agent")]
use crate::relay;
use crate::state::ViewerState;
use crate::validator;

/// The page side of the worker conversation. Data only; behavior is the free
/// functions below.
#[derive(Clone)]
pub struct Bridge {
    worker: Worker,
}

/// Spawns the worker, wires its `onmessage` to the state signals, sends `Init`
/// with the transferred canvas, and returns the bridge.
pub fn connect(offscreen: OffscreenCanvas, width: f32, height: f32, state: ViewerState) -> Bridge {
    let options = WorkerOptions::new();
    options.set_type(WorkerType::Module);
    let worker =
        Worker::new_with_options("runtime/worker.js", &options).expect("failed to spawn worker");

    #[cfg(feature = "agent")]
    let relay_socket: relay::RelaySocket = Rc::new(RefCell::new(None));
    #[cfg(feature = "agent")]
    let response_socket = relay_socket.clone();

    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        let Ok(message) = serde_wasm_bindgen::from_value::<WorkerMessage>(event.data()) else {
            return;
        };
        match message {
            WorkerMessage::Ready { context, adapter } => {
                state
                    .context
                    .set(format!("{context} (off the main thread)"));
                state.adapter.set(adapter);
                state.ready.set(true);
            }
            WorkerMessage::Stats { fps } => state.fps.set(fps),
            WorkerMessage::Camera { right, up, forward } => {
                state.camera_basis.set([right, up, forward])
            }
            WorkerMessage::Loaded {
                stats,
                clips,
                variants,
                exposure,
            } => {
                state.stats.set(Some(stats));
                state.clips.set(clips);
                state.variants.set(variants);
                state.exposure.set(exposure);
                state.active_variant.set(None);
                state.anim_clip.set(None);
                state.anim_playing.set(false);
                state.anim_time.set(0.0);
            }
            WorkerMessage::Animation {
                time,
                duration,
                playing,
                clip,
            } => {
                state.anim_time.set(time);
                state.anim_duration.set(duration);
                state.anim_playing.set(playing);
                state.anim_clip.set(clip);
            }
            WorkerMessage::Scene { nodes } => state.scene.set(nodes),
            WorkerMessage::Selected { detail } => state.selected.set(detail),
            WorkerMessage::Loading { active, label } => {
                state.loading.set(active.then_some(label));
                if active {
                    state.validation.set(None);
                }
            }
            WorkerMessage::KhronosList { entries } => state.khronos.set(entries),
            WorkerMessage::PolyhavenList { entries } => state.hdris.set(entries),
            WorkerMessage::PolyhavenModelsList { entries } => state.models.set(entries),
            #[cfg(feature = "agent")]
            WorkerMessage::Agent(response) => relay::send_response(&response_socket, &response),
        }
    });
    worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let bridge = Bridge { worker };
    send_init(&bridge, offscreen, width, height);
    send(&bridge, &ClientMessage::RefreshBrowsers);
    #[cfg(feature = "agent")]
    relay::start(bridge.clone(), relay_socket);
    bridge
}

/// Forwards a message to the worker inside the `{ message }` envelope.
pub fn send(bridge: &Bridge, message: &ClientMessage) {
    let envelope = js_sys::Object::new();
    let value = serde_wasm_bindgen::to_value(message).unwrap_or(JsValue::NULL);
    let _ = js_sys::Reflect::set(&envelope, &JsValue::from_str(MESSAGE_KEY), &value);
    let _ = bridge.worker.post_message(&envelope);
}

/// Reads a dropped or chosen file and sends its bytes to the worker. The kind
/// is inferred from the extension (`.hdr` is an environment, else a model).
pub fn send_file(bridge: &Bridge, file: File) {
    let kind = if file.name().to_lowercase().ends_with(".hdr") {
        AssetKind::Hdri
    } else {
        AssetKind::Model
    };
    let bridge = bridge.clone();
    spawn_local(async move {
        let blob: &Blob = file.as_ref();
        if let Ok(buffer) = JsFuture::from(blob.array_buffer()).await {
            let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
            send_bytes(&bridge, &ClientMessage::DropAsset { kind }, &bytes);
        }
    });
}

fn send_init(bridge: &Bridge, canvas: OffscreenCanvas, width: f32, height: f32) {
    let envelope = js_sys::Object::new();
    let value = serde_wasm_bindgen::to_value(&ClientMessage::Init { width, height })
        .unwrap_or(JsValue::NULL);
    let _ = js_sys::Reflect::set(&envelope, &JsValue::from_str(MESSAGE_KEY), &value);
    let _ = js_sys::Reflect::set(&envelope, &JsValue::from_str(CANVAS_KEY), &canvas);
    let transfer = js_sys::Array::of1(&canvas);
    let _ = bridge
        .worker
        .post_message_with_transfer(&envelope, &transfer);
}

fn send_bytes(bridge: &Bridge, message: &ClientMessage, bytes: &[u8]) {
    let array = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    array.copy_from(bytes);
    let buffer = array.buffer();

    let envelope = js_sys::Object::new();
    let value = serde_wasm_bindgen::to_value(message).unwrap_or(JsValue::NULL);
    let _ = js_sys::Reflect::set(&envelope, &JsValue::from_str(MESSAGE_KEY), &value);
    let _ = js_sys::Reflect::set(&envelope, &JsValue::from_str(BYTES_KEY), &array);
    let transfer = js_sys::Array::of1(&buffer);
    let _ = bridge
        .worker
        .post_message_with_transfer(&envelope, &transfer);
}

/// Reads everything in a drop (one or more files, or a `.zip`) and loads it: a
/// single model or HDRI directly, or a multi-file glTF as a transferred bundle.
pub fn handle_drop(bridge: &Bridge, transfer: DataTransfer, state: ViewerState) {
    let Some(files) = transfer.files() else {
        return;
    };
    if files.length() == 0 {
        return;
    }
    let bridge = bridge.clone();
    spawn_local(async move {
        let mut collected: Vec<(String, Vec<u8>)> = Vec::new();
        for index in 0..files.length() {
            if let Some(file) = files.item(index) {
                let name = file.name();
                let blob: &Blob = file.as_ref();
                if let Ok(buffer) = JsFuture::from(blob.array_buffer()).await {
                    collected.push((name, js_sys::Uint8Array::new(&buffer).to_vec()));
                }
            }
        }
        route_dropped(&bridge, collected, state);
    });
}

fn route_dropped(bridge: &Bridge, mut files: Vec<(String, Vec<u8>)>, state: ViewerState) {
    if files.len() == 1
        && files[0].0.to_lowercase().ends_with(".zip")
        && let Some(unzipped) = unzip(&files[0].1)
    {
        files = unzipped;
    }

    if let Some(index) = files.iter().position(|(name, _)| {
        let lower = name.to_lowercase();
        lower.ends_with(".glb") || lower.ends_with(".gltf")
    }) {
        let (name, gltf) = files.remove(index);
        if name.to_lowercase().ends_with(".glb") || files.is_empty() {
            validator::validate(state, gltf.clone(), Vec::new());
            send_bytes(
                bridge,
                &ClientMessage::DropAsset {
                    kind: AssetKind::Model,
                },
                &gltf,
            );
        } else {
            let directory = name
                .rsplit_once('/')
                .map(|(dir, _)| format!("{dir}/"))
                .unwrap_or_default();
            let resources: Vec<(String, Vec<u8>)> = files
                .into_iter()
                .map(|(path, bytes)| {
                    (
                        path.strip_prefix(&directory).unwrap_or(&path).to_string(),
                        bytes,
                    )
                })
                .collect();
            validator::validate(state, gltf.clone(), resources.clone());
            send_gltf_bundle(bridge, &gltf, resources);
        }
    } else if let Some(index) = files
        .iter()
        .position(|(name, _)| name.to_lowercase().ends_with(".hdr"))
    {
        let (_, bytes) = files.remove(index);
        send_bytes(
            bridge,
            &ClientMessage::DropAsset {
                kind: AssetKind::Hdri,
            },
            &bytes,
        );
    }
}

fn send_gltf_bundle(bridge: &Bridge, gltf: &[u8], resources: Vec<(String, Vec<u8>)>) {
    let envelope = js_sys::Object::new();
    let value =
        serde_wasm_bindgen::to_value(&ClientMessage::LoadGltfBundle).unwrap_or(JsValue::NULL);
    let _ = js_sys::Reflect::set(&envelope, &JsValue::from_str(MESSAGE_KEY), &value);

    let gltf_array = js_sys::Uint8Array::new_with_length(gltf.len() as u32);
    gltf_array.copy_from(gltf);
    let _ = js_sys::Reflect::set(&envelope, &JsValue::from_str(GLTF_KEY), &gltf_array);

    let transfer = js_sys::Array::new();
    transfer.push(&gltf_array.buffer());

    let resources_object = js_sys::Object::new();
    for (name, bytes) in resources {
        let array = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
        array.copy_from(&bytes);
        let _ = js_sys::Reflect::set(&resources_object, &JsValue::from_str(&name), &array);
        transfer.push(&array.buffer());
    }
    let _ = js_sys::Reflect::set(
        &envelope,
        &JsValue::from_str(RESOURCES_KEY),
        &resources_object,
    );

    let _ = bridge
        .worker
        .post_message_with_transfer(&envelope, &transfer);
}

fn unzip(bytes: &[u8]) -> Option<Vec<(String, Vec<u8>)>> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).ok()?;
    let mut out = Vec::new();
    for index in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(index) else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let mut data = Vec::new();
        if entry.read_to_end(&mut data).is_ok() {
            out.push((name, data));
        }
    }
    Some(out)
}
