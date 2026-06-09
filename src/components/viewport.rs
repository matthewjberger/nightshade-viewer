use std::collections::HashMap;

use leptos::html;
use leptos::prelude::*;
use protocol::{ClientMessage, GameCommand, GamePhase, TouchPhase};
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

/// Per-contact tracking for forwarded touches, used to tell a tap (pick) from a
/// drag (camera gesture) without echoing the engine's own touch state.
#[derive(Clone, Copy)]
struct TouchTrack {
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
    let touches = StoredValue::new(HashMap::<i32, TouchTrack>::new());
    let rect_offset = StoredValue::new((0.0_f64, 0.0_f64));

    Effect::new(move |_| {
        let Some(canvas) = canvas_ref.get() else {
            return;
        };
        if bridge.with_value(Option::is_some) {
            return;
        }
        let dpr = render_dpr() as f32;
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
        if let Some(canvas) = canvas_ref.get() {
            let rect = canvas.get_bounding_client_rect();
            rect_offset.set_value((rect.left(), rect.top()));
        }
        if event.pointer_type() == "touch" {
            let id = event.pointer_id();
            touches.update_value(|map| {
                map.insert(
                    id,
                    TouchTrack {
                        last_x: event.client_x() as f32,
                        last_y: event.client_y() as f32,
                        moved: 0.0,
                    },
                );
            });
            if let Some(canvas) = canvas_ref.get() {
                let _ = canvas.set_pointer_capture(id);
                if let Some(bridge) = bridge.get_value() {
                    let (x, y) =
                        physical(rect_offset.get_value(), event.client_x(), event.client_y());
                    send(
                        &bridge,
                        &ClientMessage::Touch {
                            id: id as u64,
                            phase: TouchPhase::Started,
                            x,
                            y,
                        },
                    );
                }
            }
            state.grabbing.set(true);
            return;
        }
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
                let (x, y) = physical(rect_offset.get_value(), event.client_x(), event.client_y());
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
        if event.pointer_type() == "touch" {
            let id = event.pointer_id();
            touches.update_value(|map| {
                if let Some(track) = map.get_mut(&id) {
                    let x = event.client_x() as f32;
                    let y = event.client_y() as f32;
                    track.moved += (x - track.last_x).abs() + (y - track.last_y).abs();
                    track.last_x = x;
                    track.last_y = y;
                }
            });
            if let Some(bridge) = bridge.get_value() {
                let (x, y) = physical(rect_offset.get_value(), event.client_x(), event.client_y());
                send(
                    &bridge,
                    &ClientMessage::Touch {
                        id: id as u64,
                        phase: TouchPhase::Moved,
                        x,
                        y,
                    },
                );
            }
            return;
        }
        drag.update_value(|state| {
            let x = event.client_x() as f32;
            let y = event.client_y() as f32;
            state.moved += (x - state.last_x).abs() + (y - state.last_y).abs();
            state.last_x = x;
            state.last_y = y;
        });
        if let Some(bridge) = bridge.get_value() {
            let (x, y) = physical(rect_offset.get_value(), event.client_x(), event.client_y());
            send(&bridge, &ClientMessage::PointerMove { x, y });
        }
    };

    let on_pointerup = move |event: PointerEvent| {
        if event.pointer_type() == "touch" {
            let id = event.pointer_id();
            let (moved, count) = touches.with_value(|map| {
                (
                    map.get(&id).map(|track| track.moved).unwrap_or(0.0),
                    map.len(),
                )
            });
            touches.update_value(|map| {
                map.remove(&id);
            });
            if let Some(canvas) = canvas_ref.get() {
                let _ = canvas.release_pointer_capture(id);
                if let Some(bridge) = bridge.get_value() {
                    let (x, y) =
                        physical(rect_offset.get_value(), event.client_x(), event.client_y());
                    send(
                        &bridge,
                        &ClientMessage::Touch {
                            id: id as u64,
                            phase: TouchPhase::Ended,
                            x,
                            y,
                        },
                    );
                    if count == 1 && moved < 5.0 {
                        send(&bridge, &tap_message(state, x, y));
                    }
                }
            }
            if touches.with_value(|map| map.is_empty()) {
                state.grabbing.set(false);
            }
            return;
        }
        let (button, moved) = drag.with_value(|state| (state.button, state.moved));
        drag.update_value(|state| state.button = None);
        state.grabbing.set(false);
        if let Some(canvas) = canvas_ref.get() {
            let _ = canvas.release_pointer_capture(event.pointer_id());
            if let Some(bridge) = bridge.get_value() {
                let (x, y) = physical(rect_offset.get_value(), event.client_x(), event.client_y());
                send(
                    &bridge,
                    &ClientMessage::PointerButton {
                        button: event.button().max(0) as u8,
                        pressed: false,
                    },
                );
                if button == Some(0) && moved < 5.0 {
                    send(&bridge, &tap_message(state, x, y));
                }
            }
        }
    };

    let on_pointercancel = move |event: PointerEvent| {
        if event.pointer_type() != "touch" {
            return;
        }
        let id = event.pointer_id();
        touches.update_value(|map| {
            map.remove(&id);
        });
        if let Some(canvas) = canvas_ref.get() {
            let _ = canvas.release_pointer_capture(id);
            if let Some(bridge) = bridge.get_value() {
                let (x, y) = physical(rect_offset.get_value(), event.client_x(), event.client_y());
                send(
                    &bridge,
                    &ClientMessage::Touch {
                        id: id as u64,
                        phase: TouchPhase::Cancelled,
                        x,
                        y,
                    },
                );
            }
        }
        if touches.with_value(HashMap::is_empty) {
            state.grabbing.set(false);
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
        if state.game_phase.get_untracked() != GamePhase::Idle {
            return;
        }
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
                on:pointercancel=on_pointercancel
                on:contextmenu=on_contextmenu
            ></canvas>
        </div>
    }
}

/// A click without drag fires a cannonball while the siege game is playing and
/// picks otherwise.
fn tap_message(state: ViewerState, x: f32, y: f32) -> ClientMessage {
    if state.game_phase.get_untracked() == GamePhase::Playing {
        ClientMessage::Game {
            command: GameCommand::Fire { x, y },
        }
    } else {
        ClientMessage::Pick { x, y }
    }
}

const MAX_RENDER_DPR: f64 = 2.0;

fn render_dpr() -> f64 {
    web_sys::window()
        .unwrap()
        .device_pixel_ratio()
        .min(MAX_RENDER_DPR)
}

/// Maps a client-space pointer position to physical canvas pixels using the
/// canvas offset captured at gesture start, so the input hot path never forces a
/// synchronous layout (a `getBoundingClientRect` per move stutters multi-touch
/// gestures, where every contact fires its own move stream).
fn physical(offset: (f64, f64), client_x: i32, client_y: i32) -> (f32, f32) {
    let dpr = render_dpr();
    (
        ((client_x as f64 - offset.0) * dpr) as f32,
        ((client_y as f64 - offset.1) * dpr) as f32,
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
    let resize_canvas = canvas.clone();
    let on_resize = Closure::<dyn FnMut()>::new(move || {
        let dpr = render_dpr() as f32;
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
