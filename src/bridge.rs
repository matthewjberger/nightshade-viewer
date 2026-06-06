use leptos::prelude::*;
use protocol::{AssetKind, BYTES_KEY, CANVAS_KEY, ClientMessage, MESSAGE_KEY, WorkerMessage};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Blob, File, MessageEvent, OffscreenCanvas, Worker, WorkerOptions, WorkerType};

use crate::state::ViewerState;

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
            }
            WorkerMessage::Stats { fps } => state.fps.set(fps),
            WorkerMessage::Scene { nodes } => state.scene.set(nodes),
            WorkerMessage::Selected { detail } => state.selected.set(detail),
            WorkerMessage::Loading { active, label } => {
                state.loading.set(active.then_some(label));
            }
            WorkerMessage::KhronosList { entries } => state.khronos.set(entries),
            WorkerMessage::PolyhavenList { entries } => state.hdris.set(entries),
            WorkerMessage::PolyhavenModelsList { entries } => state.models.set(entries),
        }
    });
    worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let bridge = Bridge { worker };
    send_init(&bridge, offscreen, width, height);
    send(&bridge, &ClientMessage::RefreshBrowsers);
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
