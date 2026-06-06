use leptos::prelude::*;
use protocol::{ClientMessage, LightKind, PrimitiveKind};
use web_sys::MouseEvent;

use crate::bridge::{Bridge, send};
use crate::state::ViewerState;

type BridgeSlot = StoredValue<Option<Bridge>, LocalStorage>;

const ITEM: &str = "px-3 py-2 rounded-lg border border-white/10 bg-black/30 text-[12px] text-white/80 hover:border-orange-400/60 hover:bg-white/5 transition-colors";

/// A modal palette for spawning primitive geometry and lights into the scene.
#[component]
pub fn AddMenu(bridge: BridgeSlot, state: ViewerState) -> impl IntoView {
    let send_msg = move |message: ClientMessage| {
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &message);
        }
        state.add_open.set(false);
    };

    view! {
        <Show when=move || state.add_open.get() fallback=|| ()>
            <div
                class="fixed inset-0 z-20 flex items-center justify-center bg-black/60 backdrop-blur-sm p-6"
                on:click=move |_| state.add_open.set(false)
            >
                <div
                    class="w-full max-w-sm flex flex-col rounded-2xl border border-white/10 bg-[#111319] shadow-2xl shadow-black/60 overflow-hidden"
                    on:click=move |event: MouseEvent| event.stop_propagation()
                >
                    <div class="px-4 py-3 border-b border-white/10 text-[14px] font-semibold text-white/90">
                        "Add"
                    </div>
                    <div class="p-4 space-y-4">
                        <div class="space-y-2">
                            <div class="text-[11px] uppercase tracking-wider text-white/40">
                                "Geometry"
                            </div>
                            <div class="grid grid-cols-3 gap-2">
                                {primitive_button(send_msg, PrimitiveKind::Cube, "Cube")}
                                {primitive_button(send_msg, PrimitiveKind::Sphere, "Sphere")}
                                {primitive_button(send_msg, PrimitiveKind::Cylinder, "Cylinder")}
                                {primitive_button(send_msg, PrimitiveKind::Cone, "Cone")}
                                {primitive_button(send_msg, PrimitiveKind::Torus, "Torus")}
                                {primitive_button(send_msg, PrimitiveKind::Plane, "Plane")}
                            </div>
                        </div>
                        <div class="space-y-2">
                            <div class="text-[11px] uppercase tracking-wider text-white/40">
                                "Lights"
                            </div>
                            <div class="grid grid-cols-3 gap-2">
                                {light_button(send_msg, LightKind::Directional, "Directional")}
                                {light_button(send_msg, LightKind::Point, "Point")}
                                {light_button(send_msg, LightKind::Spot, "Spot")}
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </Show>
    }
}

fn primitive_button(
    send_msg: impl Fn(ClientMessage) + Copy + 'static,
    kind: PrimitiveKind,
    label: &'static str,
) -> impl IntoView {
    view! {
        <button class=ITEM on:click=move |_| send_msg(ClientMessage::AddPrimitive { kind })>
            {label}
        </button>
    }
}

fn light_button(
    send_msg: impl Fn(ClientMessage) + Copy + 'static,
    kind: LightKind,
    label: &'static str,
) -> impl IntoView {
    view! {
        <button class=ITEM on:click=move |_| send_msg(ClientMessage::AddLight { kind })>
            {label}
        </button>
    }
}
