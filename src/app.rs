use leptos::prelude::*;
use wasm_bindgen::JsValue;

use crate::bridge::Bridge;
use crate::components::browser::AssetBrowser;
use crate::components::gizmo_panel::GizmoPanel;
use crate::components::inspector::Inspector;
use crate::components::loader::Loader;
use crate::components::nav_gizmo::NavGizmo;
use crate::components::scene_tree::SceneTree;
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

    view! {
        <Viewport bridge state />
        <Toolbar bridge state />
        <SceneTree bridge state />
        <Inspector bridge state />
        <NavGizmo bridge state />
        <GizmoPanel bridge state />
        <AssetBrowser bridge state />
        <Loader state />
        <Show when=move || state.dragging.get() fallback=|| ()>
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
