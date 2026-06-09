use leptos::prelude::*;
use wasm_bindgen::{JsCast, JsValue};

use protocol::{ClientMessage, GamePhase};

use crate::bridge::{Bridge, send};
use crate::components::add_menu::AddMenu;
use crate::components::animation_bar::AnimationBar;
use crate::components::browser::AssetBrowser;
use crate::components::game::Game;
use crate::components::gizmo_panel::GizmoPanel;
use crate::components::hint::Hint;
use crate::components::inspector::Inspector;
use crate::components::left_panel::LeftPanel;
use crate::components::loader::Loader;
use crate::components::nav_gizmo::NavGizmo;
use crate::components::toolbar::Toolbar;
use crate::components::viewport::Viewport;
use crate::state::ViewerState;

/// Application root: owns the shared state and bridge slot, and composes the
/// viewport, panels, asset browser, and drag overlay. Falls back to a notice
/// when the browser has no WebGPU.
#[component]
pub fn App() -> impl IntoView {
    if !webgpu_supported() {
        return unsupported().into_any();
    }

    let state = ViewerState::new();
    let bridge = StoredValue::new_local(None::<Bridge>);

    let _ = window_event_listener(leptos::ev::keydown, move |event| {
        if typing_in_field(&event) {
            return;
        }
        match event.key().as_str() {
            "h" | "H" => state.ui_hidden.update(|hidden| *hidden = !*hidden),
            "Escape" => state.ui_hidden.set(false),
            "x" | "X" => {
                if let Some(bridge) = bridge.get_value() {
                    send(&bridge, &ClientMessage::DeleteSelected);
                }
            }
            _ => {}
        }
    });

    let game_idle = move || state.game_phase.get() == GamePhase::Idle;

    view! {
        <Viewport bridge state />
        <Show when=move || !state.ui_hidden.get() && game_idle() fallback=|| ()>
            <Toolbar bridge state />
            <Hint state />
            <LeftPanel bridge state />
            <Inspector bridge state />
            <NavGizmo bridge state />
            <GizmoPanel bridge state />
            <AnimationBar bridge state />
            <AssetBrowser bridge state />
            <AddMenu bridge state />
        </Show>
        <Game bridge state />
        <Loader state />
        <Show when=move || state.ui_hidden.get() && game_idle() fallback=|| ()>
            <button
                class="fixed bottom-3 right-3 z-40 w-2.5 h-2.5 rounded-full bg-white/25 hover:bg-white/70 transition-colors"
                title="Show interface (H)"
                on:click=move |_| state.ui_hidden.set(false)
            ></button>
        </Show>
        <Show when=move || state.dragging.get() && game_idle() fallback=|| ()>
            <div class="fixed inset-2 z-30 pointer-events-none flex items-center justify-center rounded-2xl border-4 border-dashed border-orange-400/50 bg-orange-500/10">
                <div class="px-6 py-4 rounded-xl bg-[#14161d]/90 border border-white/10 text-white/90 text-[15px] shadow-2xl">
                    "Drop a .glb, .gltf, or .hdr file"
                </div>
            </div>
        </Show>
    }
    .into_any()
}

fn unsupported() -> impl IntoView {
    view! {
        <div class="fixed inset-0 flex items-center justify-center p-8 text-center text-[#9aa0b4] bg-[#0c0d12]">
            <div class="max-w-[420px]">
                <h1 class="text-[16px] text-[#cfd3e0] mb-2">"WebGPU not available"</h1>
                <p>
                    "This viewer runs the Nightshade engine in a web worker through WebGPU. Open it in a browser with WebGPU and OffscreenCanvas-in-workers support (Chromium 113+, Firefox 141+)."
                </p>
            </div>
        </div>
    }
}

fn typing_in_field(event: &web_sys::KeyboardEvent) -> bool {
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok())
        .map(|element| {
            let tag = element.tag_name();
            tag.eq_ignore_ascii_case("input")
                || tag.eq_ignore_ascii_case("textarea")
                || tag.eq_ignore_ascii_case("select")
                || element.is_content_editable()
        })
        .unwrap_or(false)
}

fn webgpu_supported() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(navigator) = js_sys::Reflect::get(window.as_ref(), &JsValue::from_str("navigator"))
    else {
        return false;
    };
    js_sys::Reflect::get(&navigator, &JsValue::from_str("gpu"))
        .map(|gpu| !gpu.is_undefined() && !gpu.is_null())
        .unwrap_or(false)
}
