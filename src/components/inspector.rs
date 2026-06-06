use leptos::prelude::*;
use protocol::ClientMessage;
use wasm_bindgen::JsCast;
use web_sys::{Element, Event, PointerEvent};

use crate::bridge::{Bridge, send};
use crate::state::ViewerState;

/// The right panel: the selected entity's name, mesh, and an editable
/// translation / rotation / scale.
#[component]
pub fn Inspector(
    bridge: StoredValue<Option<Bridge>, LocalStorage>,
    state: ViewerState,
) -> impl IntoView {
    let id = RwSignal::new(0u32);
    let name = RwSignal::new(String::new());
    let mesh = RwSignal::new(None::<String>);
    let translation = [
        RwSignal::new(0.0f32),
        RwSignal::new(0.0f32),
        RwSignal::new(0.0f32),
    ];
    let rotation = [
        RwSignal::new(0.0f32),
        RwSignal::new(0.0f32),
        RwSignal::new(0.0f32),
    ];
    let scale = [
        RwSignal::new(1.0f32),
        RwSignal::new(1.0f32),
        RwSignal::new(1.0f32),
    ];

    Effect::new(move |_| {
        if let Some(detail) = state.selected.get() {
            id.set(detail.id);
            name.set(detail.name);
            mesh.set(detail.mesh);
            for axis in 0..3 {
                translation[axis].set(detail.translation[axis]);
                rotation[axis].set(detail.rotation[axis]);
                scale[axis].set(detail.scale[axis]);
            }
        }
    });

    let push = move || {
        if let Some(bridge) = bridge.get_value() {
            send(
                &bridge,
                &ClientMessage::SetTransform {
                    id: id.get_untracked(),
                    translation: read(translation),
                    rotation: read(rotation),
                    scale: read(scale),
                },
            );
        }
    };

    let panel_class = move || {
        let base = "fixed top-16 right-3 bottom-3 w-72 max-w-[85vw] z-20 flex flex-col rounded-xl border border-white/10 bg-[#14161d]/85 backdrop-blur-md shadow-2xl shadow-black/40 overflow-hidden transition-transform duration-200 sm:translate-x-0";
        if state.inspector_open.get() {
            format!("{base} translate-x-0")
        } else {
            format!("{base} translate-x-[120%]")
        }
    };

    view! {
        <Show when=move || state.inspector_open.get() fallback=|| ()>
            <div
                class="fixed inset-0 z-10 bg-black/40 sm:hidden"
                on:click=move |_| state.inspector_open.set(false)
            ></div>
        </Show>
        <div class=panel_class>
            <div class="flex items-center justify-between px-3 py-2.5 text-[11px] font-semibold uppercase tracking-wider text-white/50 border-b border-white/10">
                <span>"Inspector"</span>
                <button
                    class="sm:hidden text-white/40 hover:text-white/80 text-[14px] leading-none px-1"
                    on:click=move |_| state.inspector_open.set(false)
                >
                    "✕"
                </button>
            </div>
            <Show
                when=move || state.selected.get().is_some()
                fallback=|| {
                    view! {
                        <div class="px-3 py-5 text-[12px] text-white/35">
                            "Select an object in the scene or click it in the viewport."
                        </div>
                    }
                }
            >
                <div class="flex-1 overflow-y-auto">
                    <div class="px-3 py-2.5 border-b border-white/10">
                        <div class="text-[13px] text-white/90 font-medium truncate">
                            {move || name.get()}
                        </div>
                        <Show when=move || mesh.get().is_some() fallback=|| ()>
                            <div class="text-[11px] text-sky-300/70 truncate">
                                {move || mesh.get().unwrap_or_default()}
                            </div>
                        </Show>
                    </div>
                    {vec3_field("Position", translation, 0.01, push)}
                    {vec3_field("Rotation", rotation, 0.5, push)}
                    {vec3_field("Scale", scale, 0.01, push)}
                </div>
            </Show>
        </div>
    }
}

fn read(fields: [RwSignal<f32>; 3]) -> [f32; 3] {
    [
        fields[0].get_untracked(),
        fields[1].get_untracked(),
        fields[2].get_untracked(),
    ]
}

fn vec3_field(
    label: &'static str,
    fields: [RwSignal<f32>; 3],
    step: f32,
    push: impl Fn() + Copy + 'static,
) -> impl IntoView {
    view! {
        <div class="px-3 py-2 border-b border-white/5">
            <div class="text-[11px] uppercase tracking-wider text-white/40 mb-1.5">{label}</div>
            <div class="grid grid-cols-3 gap-1.5">
                {["X", "Y", "Z"]
                    .into_iter()
                    .enumerate()
                    .map(|(axis, glyph)| {
                        let signal = fields[axis];
                        let drag = StoredValue::new(None::<(f32, f32)>);
                        let on_down = move |event: PointerEvent| {
                            event.prevent_default();
                            drag.set_value(Some((signal.get_untracked(), event.client_x() as f32)));
                            if let Some(element) = pointer_target(&event) {
                                let _ = element.set_pointer_capture(event.pointer_id());
                            }
                        };
                        let on_move = move |event: PointerEvent| {
                            if let Some((start_value, start_x)) = drag.get_value() {
                                signal.set(start_value + (event.client_x() as f32 - start_x) * step);
                                push();
                            }
                        };
                        let on_up = move |event: PointerEvent| {
                            drag.set_value(None);
                            if let Some(element) = pointer_target(&event) {
                                let _ = element.release_pointer_capture(event.pointer_id());
                            }
                        };
                        view! {
                            <label class="flex items-center gap-1 rounded-md bg-black/30 border border-white/10 px-1.5 focus-within:border-orange-400/60">
                                <span
                                    class="text-[10px] text-white/35 cursor-ew-resize select-none px-0.5 hover:text-orange-300"
                                    on:pointerdown=on_down
                                    on:pointermove=on_move
                                    on:pointerup=on_up
                                >
                                    {glyph}
                                </span>
                                <input
                                    type="number"
                                    step="0.05"
                                    class="w-full bg-transparent py-1 text-[12px] text-white/90 outline-none tabular-nums"
                                    prop:value=move || format!("{:.3}", signal.get())
                                    on:input=move |event| {
                                        if let Ok(value) = input_value(&event).parse::<f32>() {
                                            signal.set(value);
                                            push();
                                        }
                                    }
                                />
                            </label>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}

fn pointer_target(event: &PointerEvent) -> Option<Element> {
    event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok())
}

fn input_value(event: &Event) -> String {
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|input| input.value())
        .unwrap_or_default()
}
