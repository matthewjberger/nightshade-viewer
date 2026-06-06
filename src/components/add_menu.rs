use leptos::html;
use leptos::prelude::*;
use protocol::{ClientMessage, LightKind, PrimitiveKind};
use web_sys::{Event, HtmlElement, MouseEvent};

use crate::bridge::{Bridge, send, send_file};
use crate::state::ViewerState;

type BridgeSlot = StoredValue<Option<Bridge>, LocalStorage>;

const ITEM: &str = "px-3 py-2 rounded-lg border border-white/10 bg-black/30 text-[12px] text-white/80 hover:border-orange-400/60 hover:bg-white/5 transition-colors";

/// Create content in the scene: primitive geometry, lights, or a local file.
#[component]
pub fn AddMenu(bridge: BridgeSlot, state: ViewerState) -> impl IntoView {
    let file_ref = NodeRef::<html::Input>::new();
    let on_file = move |_event: Event| {
        if let (Some(input), Some(bridge)) = (file_ref.get(), bridge.get_value())
            && let Some(files) = input.files()
            && let Some(file) = files.item(0)
        {
            send_file(&bridge, file);
        }
        state.add_open.set(false);
    };
    let import_file = move |_| {
        if let Some(input) = file_ref.get() {
            let element: &HtmlElement = input.as_ref();
            element.click();
        }
    };

    // Geometry and lights are editable entities, so opening the inspector after
    // adding one closes the loop without auto-opening it on a viewport pick.
    let add_entity = move |message: ClientMessage| {
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &message);
        }
        state.inspector_open.set(true);
        state.add_open.set(false);
    };

    view! {
        <input
            type="file"
            node_ref=file_ref
            accept=".glb,.gltf,.hdr"
            class="hidden"
            on:change=on_file
        />
        <Show when=move || state.add_open.get() fallback=|| ()>
            <div
                class="fixed inset-0 z-20 flex items-center justify-center bg-black/60 backdrop-blur-sm p-6"
                on:click=move |_| state.add_open.set(false)
            >
                <div
                    class="w-full max-w-sm max-h-[82vh] flex flex-col rounded-2xl border border-white/10 bg-[#111319] shadow-2xl shadow-black/60 overflow-hidden"
                    on:click=move |event: MouseEvent| event.stop_propagation()
                >
                    <div class="px-4 py-3 border-b border-white/10 text-[14px] font-semibold text-white/90">
                        "Add"
                    </div>
                    <div class="p-4 space-y-4 overflow-y-auto">
                        <div class="space-y-2">
                            <div class="text-[11px] uppercase tracking-wider text-white/40">
                                "Geometry"
                            </div>
                            <div class="grid grid-cols-2 sm:grid-cols-3 gap-2">
                                {primitive_button(add_entity, PrimitiveKind::Cube, "Cube")}
                                {primitive_button(add_entity, PrimitiveKind::Sphere, "Sphere")}
                                {primitive_button(add_entity, PrimitiveKind::Cylinder, "Cylinder")}
                                {primitive_button(add_entity, PrimitiveKind::Cone, "Cone")}
                                {primitive_button(add_entity, PrimitiveKind::Torus, "Torus")}
                                {primitive_button(add_entity, PrimitiveKind::Plane, "Plane")}
                            </div>
                        </div>
                        <div class="space-y-2">
                            <div class="text-[11px] uppercase tracking-wider text-white/40">
                                "Lights"
                            </div>
                            <div class="grid grid-cols-2 sm:grid-cols-3 gap-2">
                                {light_button(add_entity, LightKind::Directional, "Directional")}
                                {light_button(add_entity, LightKind::Point, "Point")}
                                {light_button(add_entity, LightKind::Spot, "Spot")}
                            </div>
                        </div>
                        <div class="space-y-2">
                            <div class="text-[11px] uppercase tracking-wider text-white/40">
                                "Import"
                            </div>
                            <button class=format!("{ITEM} w-full") on:click=import_file>
                                "Import file…"
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </Show>
    }
}

fn primitive_button(
    add: impl Fn(ClientMessage) + Copy + 'static,
    kind: PrimitiveKind,
    label: &'static str,
) -> impl IntoView {
    view! {
        <button class=ITEM on:click=move |_| add(ClientMessage::AddPrimitive { kind })>
            {label}
        </button>
    }
}

fn light_button(
    add: impl Fn(ClientMessage) + Copy + 'static,
    kind: LightKind,
    label: &'static str,
) -> impl IntoView {
    view! {
        <button class=ITEM on:click=move |_| add(ClientMessage::AddLight { kind })>
            {label}
        </button>
    }
}
