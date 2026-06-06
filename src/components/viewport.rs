use leptos::html;
use leptos::prelude::*;
use protocol::ClientMessage;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{DragEvent, HtmlCanvasElement, MouseEvent, PointerEvent, ResizeObserver, WheelEvent};

use crate::bridge::{self, Bridge, send};
use crate::state::ViewerState;

#[derive(Clone, Copy, Default)]
struct DragState {
    button: Option<u8>,
    last_x: f32,
    last_y: f32,
    moved: f32,
}

/// The render surface. Forwards raw pointer and wheel input to the worker so the
/// engine drives the camera and gizmos, detects a click (no drag) to pick, and
/// accepts dropped files.
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
        attach_wheel(&canvas, bridge);
        observe_resize(canvas, connected.clone());
        bridge.set_value(Some(connected));
    });

    let on_pointerdown = move |event: PointerEvent| {
        let button = event.button().max(0) as u8;
        drag.update_value(|state| {
            state.button = Some(button);
            state.last_x = event.client_x() as f32;
            state.last_y = event.client_y() as f32;
            state.moved = 0.0;
        });
        if let Some(canvas) = canvas_ref.get() {
            let _ = canvas.set_pointer_capture(event.pointer_id());
            if let Some(bridge) = bridge.get_value() {
                let (x, y) = physical(&canvas, event.client_x(), event.client_y());
                send(&bridge, &ClientMessage::PointerMove { x, y });
                send(
                    &bridge,
                    &ClientMessage::PointerButton {
                        button,
                        pressed: true,
                    },
                );
            }
        }
        state.grabbing.set(true);
    };

    let on_pointermove = move |event: PointerEvent| {
        drag.update_value(|state| {
            let x = event.client_x() as f32;
            let y = event.client_y() as f32;
            state.moved += (x - state.last_x).abs() + (y - state.last_y).abs();
            state.last_x = x;
            state.last_y = y;
        });
        if let Some(canvas) = canvas_ref.get()
            && let Some(bridge) = bridge.get_value()
        {
            let (x, y) = physical(&canvas, event.client_x(), event.client_y());
            send(&bridge, &ClientMessage::PointerMove { x, y });
        }
    };

    let on_pointerup = move |event: PointerEvent| {
        let (button, moved) = drag.with_value(|state| (state.button, state.moved));
        drag.update_value(|state| state.button = None);
        state.grabbing.set(false);
        if let Some(canvas) = canvas_ref.get() {
            let _ = canvas.release_pointer_capture(event.pointer_id());
            if let Some(bridge) = bridge.get_value() {
                let (x, y) = physical(&canvas, event.client_x(), event.client_y());
                send(
                    &bridge,
                    &ClientMessage::PointerButton {
                        button: event.button().max(0) as u8,
                        pressed: false,
                    },
                );
                if button == Some(0) && moved < 5.0 {
                    send(&bridge, &ClientMessage::Pick { x, y });
                }
            }
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
        if let (Some(transfer), Some(bridge)) = (event.data_transfer(), bridge.get_value()) {
            bridge::handle_drop(&bridge, transfer, state);
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
                on:contextmenu=on_contextmenu
            ></canvas>
        </div>
    }
}

fn physical(canvas: &HtmlCanvasElement, client_x: i32, client_y: i32) -> (f32, f32) {
    let dpr = web_sys::window().unwrap().device_pixel_ratio();
    let rect = canvas.get_bounding_client_rect();
    (
        ((client_x as f64 - rect.left()) * dpr) as f32,
        ((client_y as f64 - rect.top()) * dpr) as f32,
    )
}

fn attach_wheel(canvas: &HtmlCanvasElement, bridge: StoredValue<Option<Bridge>, LocalStorage>) {
    let on_wheel = Closure::<dyn FnMut(WheelEvent)>::new(move |event: WheelEvent| {
        event.prevent_default();
        if let Some(bridge) = bridge.get_value() {
            send(
                &bridge,
                &ClientMessage::Wheel {
                    delta: event.delta_y() as f32,
                },
            );
        }
    });
    let options = web_sys::AddEventListenerOptions::new();
    options.set_passive(false);
    canvas
        .add_event_listener_with_callback_and_add_event_listener_options(
            "wheel",
            on_wheel.as_ref().unchecked_ref(),
            &options,
        )
        .expect("failed to add wheel listener");
    on_wheel.forget();
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
