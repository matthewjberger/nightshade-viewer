use leptos::prelude::*;

use crate::state::ViewerState;

const BUTTON: &str =
    "shrink-0 px-2.5 py-1 rounded-md text-[12px] text-white/80 hover:bg-white/10 transition-colors";

/// The top bar: the scene and inspector panel toggles, the Add menu, and a live
/// status readout.
#[component]
pub fn Toolbar(state: ViewerState) -> impl IntoView {
    let toggle_scene = move |_| {
        let open = !state.scene_open.get_untracked();
        state.scene_open.set(open);
        if open && is_mobile() {
            state.inspector_open.set(false);
        }
    };
    let toggle_inspector = move |_| {
        let open = !state.inspector_open.get_untracked();
        state.inspector_open.set(open);
        if open && is_mobile() {
            state.scene_open.set(false);
        }
    };
    let scene_class = move || {
        if state.scene_open.get() {
            format!("{BUTTON} bg-white/10 text-white")
        } else {
            BUTTON.to_string()
        }
    };
    let inspector_class = move || {
        if state.inspector_open.get() {
            format!("{BUTTON} bg-white/10 text-white")
        } else {
            BUTTON.to_string()
        }
    };
    let status = move || match state.loading.get() {
        Some(label) => format!("Loading {label}…"),
        None => format!("{:.0} fps", state.fps.get()),
    };

    view! {
        <div class="fixed top-3 left-3 right-3 h-10 z-10 flex items-center gap-1 px-3 rounded-xl border border-white/10 bg-[#14161d]/85 backdrop-blur-md shadow-lg shadow-black/40">
            <span class="shrink min-w-0 truncate text-[13px] font-semibold text-white/90 mr-1 sm:mr-3">
                "Nightshade"
            </span>
            <button class=scene_class on:click=toggle_scene>"Scene"</button>
            <button class=inspector_class on:click=toggle_inspector>"Inspect"</button>
            <button class=BUTTON on:click=move |_| state.add_open.set(true)>"+ Add"</button>
            <div class="flex-1"></div>
            <span class="hidden sm:inline shrink-0 text-[11px] text-white/45 tabular-nums pl-1">
                {status}
            </span>
        </div>
    }
}

fn is_mobile() -> bool {
    web_sys::window()
        .and_then(|window| window.inner_width().ok())
        .and_then(|value| value.as_f64())
        .map(|width| width < 640.0)
        .unwrap_or(false)
}
