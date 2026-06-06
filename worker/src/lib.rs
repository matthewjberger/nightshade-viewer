mod ecs;
mod state;
mod systems;

use std::cell::RefCell;
use std::rc::Rc;

use nightshade::prelude::winit::event::{
    ElementState as WinitElementState, MouseButton as WinitMouseButton,
};
use nightshade::prelude::*;
use nightshade::render::wgpu::create_wgpu_renderer;
use protocol::{AssetKind, BYTES_KEY, ClientMessage, GizmoKind, MESSAGE_KEY, WorkerMessage};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent, OffscreenCanvas};

use crate::ecs::PendingAsset;
use crate::state::Viewer;

type AppSlot = Rc<RefCell<Option<App>>>;

struct App {
    world: World,
    renderer: WgpuRenderer,
    state: Viewer,
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();

    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let app_slot: AppSlot = Rc::new(RefCell::new(None));

    let handler_scope = scope.clone();
    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        handle_message(&handler_scope, &app_slot, event);
    });
    scope.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
}

fn handle_message(scope: &DedicatedWorkerGlobalScope, app_slot: &AppSlot, event: MessageEvent) {
    let data = event.data();
    let Ok(payload) = js_sys::Reflect::get(&data, &JsValue::from_str(MESSAGE_KEY)) else {
        return;
    };
    let Ok(message) = serde_wasm_bindgen::from_value::<ClientMessage>(payload) else {
        return;
    };

    match message {
        ClientMessage::Init { width, height } => {
            let Some(canvas) = canvas_from(&data) else {
                return;
            };
            let scope = scope.clone();
            let app_slot = app_slot.clone();
            spawn_local(async move {
                let app = create_app(canvas, width, height).await;
                post(&WorkerMessage::Ready {
                    context: context(),
                    adapter: "WebGPU".to_string(),
                });
                *app_slot.borrow_mut() = Some(app);
                start_render_loop(scope, app_slot);
            });
        }
        ClientMessage::Resize { width, height } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                resize_offscreen(
                    &mut app.world,
                    &mut app.renderer,
                    (width as u32).max(1),
                    (height as u32).max(1),
                );
            }
        }
        ClientMessage::PointerMove { x, y } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                input_inject_cursor_moved(&mut app.world, Vec2::new(x, y));
            }
        }
        ClientMessage::PointerButton { button, pressed } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                let state = if pressed {
                    WinitElementState::Pressed
                } else {
                    WinitElementState::Released
                };
                input_inject_mouse_button(&mut app.world, mouse_button(button), state);
            }
        }
        ClientMessage::Wheel { delta } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                input_inject_mouse_wheel(&mut app.world, Vec2::new(0.0, -delta / 100.0));
            }
        }
        ClientMessage::Pick { x, y } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::picking::request(&mut app.state.viewer, &mut app.world, x, y);
            }
        }
        ClientMessage::Select { id } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::selection::select_by_id(&mut app.state.viewer, &mut app.world, id);
            }
        }
        ClientMessage::Deselect => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::selection::select(&mut app.state.viewer, &mut app.world, None);
            }
        }
        ClientMessage::SetTransform {
            id,
            translation,
            rotation,
            scale,
        } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::selection::set_transform(&mut app.world, id, translation, rotation, scale);
            }
        }
        ClientMessage::SetGizmoMode { mode } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                app.world.resources.user_interface.gizmos.mode = match mode {
                    GizmoKind::Translate => nightshade::ecs::gizmos::GizmoMode::LocalTranslation,
                    GizmoKind::Rotate => nightshade::ecs::gizmos::GizmoMode::Rotation,
                    GizmoKind::Scale => nightshade::ecs::gizmos::GizmoMode::Scale,
                };
            }
        }
        ClientMessage::Frame => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                app.state.viewer.resources.camera_input.frame_requested = true;
            }
        }
        ClientMessage::DropAsset { kind } => {
            if let Some(app) = app_slot.borrow_mut().as_mut()
                && let Some(bytes) = bytes_from(&data)
                && let Ok(mut slot) = app.state.viewer.resources.incoming.asset.lock()
            {
                *slot = Some(match kind {
                    AssetKind::Model => PendingAsset::Model(bytes),
                    AssetKind::Hdri => PendingAsset::Hdri(bytes),
                });
            }
        }
        ClientMessage::LoadKhronos { name } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::browsers::fetch_khronos(&app.state.viewer, &name);
            }
        }
        ClientMessage::LoadPolyhaven { slug } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::browsers::fetch_polyhaven(&app.state.viewer, &slug);
            }
        }
        ClientMessage::LoadPolyhavenModel { slug } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::browsers::fetch_polyhaven_model(&app.state.viewer, &slug);
            }
        }
        ClientMessage::RefreshBrowsers => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::browsers::resend(&mut app.state.viewer);
            }
        }
    }
}

async fn create_app(canvas: OffscreenCanvas, width: f32, height: f32) -> App {
    let physical_width = (width as u32).max(1);
    let physical_height = (height as u32).max(1);

    let surface_target = wgpu::SurfaceTarget::OffscreenCanvas(canvas);
    let mut renderer = create_wgpu_renderer(surface_target, physical_width, physical_height)
        .await
        .expect("failed to create renderer from offscreen canvas");

    let mut world = World::default();
    let mut state = Viewer::default();
    initialize_offscreen(
        &mut world,
        &mut state,
        &mut renderer,
        (physical_width, physical_height),
        1.0,
    );

    App {
        world,
        renderer,
        state,
    }
}

fn start_render_loop(_scope: DedicatedWorkerGlobalScope, app_slot: AppSlot) {
    let last_push = Rc::new(RefCell::new(0.0_f64));

    spawn_animation_frame_loop(move || {
        if let Some(app) = app_slot.borrow_mut().as_mut() {
            tick_offscreen(&mut app.world, &mut app.state, &mut app.renderer);
            let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
            if let Some(performance) = scope.performance() {
                let now = performance.now();
                let mut last = last_push.borrow_mut();
                if now - *last > 500.0 {
                    *last = now;
                    post(&WorkerMessage::Stats {
                        fps: app.world.resources.window.timing.frames_per_second,
                    });
                }
            }
        }
    });
}

fn mouse_button(button: u8) -> WinitMouseButton {
    match button {
        1 => WinitMouseButton::Middle,
        2 => WinitMouseButton::Right,
        _ => WinitMouseButton::Left,
    }
}

fn bytes_from(data: &JsValue) -> Option<Vec<u8>> {
    let value = js_sys::Reflect::get(data, &JsValue::from_str(BYTES_KEY)).ok()?;
    let array = value.dyn_into::<js_sys::Uint8Array>().ok()?;
    Some(array.to_vec())
}

fn canvas_from(data: &JsValue) -> Option<OffscreenCanvas> {
    js_sys::Reflect::get(data, &JsValue::from_str(protocol::CANVAS_KEY))
        .ok()
        .and_then(|value| value.dyn_into::<OffscreenCanvas>().ok())
}

pub(crate) fn post(message: &WorkerMessage) {
    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    if let Ok(value) = serde_wasm_bindgen::to_value(message) {
        let _ = scope.post_message(&value);
    }
}

fn context() -> String {
    let global = js_sys::global();
    js_sys::Reflect::get(&global, &JsValue::from_str("constructor"))
        .ok()
        .and_then(|constructor| js_sys::Reflect::get(&constructor, &JsValue::from_str("name")).ok())
        .and_then(|name| name.as_string())
        .unwrap_or_else(|| "worker".to_string())
}
