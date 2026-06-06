use leptos::html;
use leptos::prelude::*;
use protocol::ClientMessage;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{DragEvent, HtmlCanvasElement, MouseEvent, PointerEvent, ResizeObserver, WheelEvent};

use crate::bridge::{self, Bridge, send, send_file};
use crate::state::ViewerState;

#[derive(Clone, Copy, PartialEq)]
enum DragMode {
    None,
    Orbit,
    Pan,
}

#[derive(Clone, Copy)]
struct DragState {
    mode: DragMode,
    last_x: f32,
    last_y: f32,
    moved: f32,
}

impl Default for DragState {
    fn default() -> Self {
        Self {
            mode: DragMode::None,
            last_x: 0.0,
            last_y: 0.0,
            moved: 0.0,
        }
    }
}

/// The render surface: transfers the canvas to the worker, forwards orbit / pan
/// / zoom / pick input, and accepts dropped files.
#[component]
pub fn Viewport(
    bridge: StoredValue<Option<Bridge>, LocalStorage>,
    state: ViewerState,
) -> impl IntoView {
    let canvas_ref = NodeRef::<html::Canvas>::new();
    let drag = StoredValue::new(DragState::default());

    Effect::new(move |_| {
        let Some(canvas) = canvas_ref.get() else {
            return;
        };
        if bridge.with_value(Option::is_some) {
            return;
        }
        let window = web_sys::window().unwrap();
        let dpr = window.device_pixel_ratio() as f32;
        let rect = canvas.get_bounding_client_rect();
        let width = rect.width() as f32 * dpr;
        let height = rect.height() as f32 * dpr;
        canvas.set_width(width as u32);
        canvas.set_height(height as u32);
        let offscreen = canvas
            .transfer_control_to_offscreen()
            .expect("failed to transfer canvas to offscreen");
        let connected = bridge::connect(offscreen, width, height, state);
        observe_resize(canvas, connected.clone());
        bridge.set_value(Some(connected));
    });

    let on_pointerdown = move |event: PointerEvent| {
        let mode = if event.button() == 0 {
            DragMode::Orbit
        } else {
            DragMode::Pan
        };
        drag.update_value(|d| {
            d.mode = mode;
            d.last_x = event.client_x() as f32;
            d.last_y = event.client_y() as f32;
            d.moved = 0.0;
        });
        if let Some(canvas) = canvas_ref.get() {
            let _ = canvas.set_pointer_capture(event.pointer_id());
        }
        state.grabbing.set(true);
    };

    let on_pointermove = move |event: PointerEvent| {
        let message = drag
            .try_update_value(|d| {
                if d.mode == DragMode::None {
                    return None;
                }
                let x = event.client_x() as f32;
                let y = event.client_y() as f32;
                let dx = x - d.last_x;
                let dy = y - d.last_y;
                d.last_x = x;
                d.last_y = y;
                d.moved += dx.abs() + dy.abs();
                match d.mode {
                    DragMode::Orbit => Some(ClientMessage::Orbit { yaw: dx, pitch: dy }),
                    DragMode::Pan => Some(ClientMessage::Pan { dx, dy }),
                    DragMode::None => None,
                }
            })
            .flatten();
        if let (Some(message), Some(bridge)) = (message, bridge.get_value()) {
            send(&bridge, &message);
        }
    };

    let on_pointerup = move |event: PointerEvent| {
        let (mode, moved) = drag.with_value(|d| (d.mode, d.moved));
        drag.update_value(|d| d.mode = DragMode::None);
        state.grabbing.set(false);
        if let Some(canvas) = canvas_ref.get() {
            let _ = canvas.release_pointer_capture(event.pointer_id());
            if mode == DragMode::Orbit
                && moved < 4.0
                && let Some(bridge) = bridge.get_value()
            {
                let dpr = web_sys::window().unwrap().device_pixel_ratio();
                let rect = canvas.get_bounding_client_rect();
                let x = (event.client_x() as f64 - rect.left()) * dpr;
                let y = (event.client_y() as f64 - rect.top()) * dpr;
                send(
                    &bridge,
                    &ClientMessage::Pick {
                        x: x as f32,
                        y: y as f32,
                    },
                );
            }
        }
    };

    let on_wheel = move |event: WheelEvent| {
        event.prevent_default();
        if let Some(bridge) = bridge.get_value() {
            let amount = (event.delta_y() as f32 / 100.0).clamp(-4.0, 4.0);
            send(&bridge, &ClientMessage::Zoom { amount });
        }
    };

    let on_contextmenu = move |event: MouseEvent| event.prevent_default();

    let on_dragover = move |event: DragEvent| {
        event.prevent_default();
        state.dragging.set(true);
    };
    let on_dragleave = move |_event: DragEvent| state.dragging.set(false);
    let on_drop = move |event: DragEvent| {
        event.prevent_default();
        state.dragging.set(false);
        if let (Some(transfer), Some(bridge)) = (event.data_transfer(), bridge.get_value())
            && let Some(files) = transfer.files()
            && let Some(file) = files.item(0)
        {
            send_file(&bridge, file);
        }
    };

    let canvas_class = move || {
        let cursor = if state.grabbing.get() {
            "cursor-grabbing"
        } else {
            "cursor-grab"
        };
        format!("block w-full h-full touch-none {cursor}")
    };

    view! {
        <div
            class="fixed inset-0 bg-[#0c0d12]"
            on:dragover=on_dragover
            on:dragleave=on_dragleave
            on:drop=on_drop
        >
            <canvas
                id="canvas"
                node_ref=canvas_ref
                class=canvas_class
                on:pointerdown=on_pointerdown
                on:pointermove=on_pointermove
                on:pointerup=on_pointerup
                on:wheel=on_wheel
                on:contextmenu=on_contextmenu
            ></canvas>
        </div>
    }
}

fn observe_resize(canvas: HtmlCanvasElement, bridge: Bridge) {
    let resize_window = web_sys::window().unwrap();
    let resize_canvas = canvas.clone();
    let on_resize = Closure::<dyn FnMut()>::new(move || {
        let dpr = resize_window.device_pixel_ratio() as f32;
        let rect = resize_canvas.get_bounding_client_rect();
        send(
            &bridge,
            &ClientMessage::Resize {
                width: rect.width() as f32 * dpr,
                height: rect.height() as f32 * dpr,
            },
        );
    });
    let observer = ResizeObserver::new(on_resize.as_ref().unchecked_ref())
        .expect("failed to create resize observer");
    observer.observe(&canvas);
    on_resize.forget();
    std::mem::forget(observer);
}
