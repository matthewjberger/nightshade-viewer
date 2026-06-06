use leptos::prelude::*;
use protocol::{ClientMessage, GizmoKind};

use crate::bridge::{Bridge, send};
use crate::state::ViewerState;

/// Floating mode switcher for the transform gizmo, shown while an entity is
/// selected. Picks translate / rotate / scale.
#[component]
pub fn GizmoPanel(
    bridge: StoredValue<Option<Bridge>, LocalStorage>,
    state: ViewerState,
) -> impl IntoView {
    let set_mode = move |mode: GizmoKind| {
        state.gizmo_mode.set(mode);
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &ClientMessage::SetGizmoMode { mode });
        }
    };

    view! {
        <Show when=move || state.selected.get().is_some() fallback=|| ()>
            <div class="fixed bottom-4 left-1/2 -translate-x-1/2 z-10 flex items-center gap-1 p-1 rounded-xl border border-white/10 bg-[#14161d]/85 backdrop-blur-md shadow-lg shadow-black/40">
                {gizmo_button(state, GizmoKind::Translate, "Move", set_mode)}
                {gizmo_button(state, GizmoKind::Rotate, "Rotate", set_mode)}
                {gizmo_button(state, GizmoKind::Scale, "Scale", set_mode)}
            </div>
        </Show>
    }
}

fn gizmo_button(
    state: ViewerState,
    mode: GizmoKind,
    label: &'static str,
    set_mode: impl Fn(GizmoKind) + Copy + 'static,
) -> impl IntoView {
    let class = move || {
        let base = "px-3 py-1.5 rounded-lg text-[12px] transition-colors";
        if state.gizmo_mode.get() == mode {
            format!("{base} bg-orange-500/25 text-orange-200")
        } else {
            format!("{base} text-white/70 hover:bg-white/10")
        }
    };
    view! {
        <button class=class on:click=move |_| set_mode(mode)>
            {label}
        </button>
    }
}
