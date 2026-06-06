use leptos::prelude::*;
use protocol::ClientMessage;

use crate::bridge::{Bridge, send};
use crate::state::ViewerState;

type BridgeSlot = StoredValue<Option<Bridge>, LocalStorage>;

const BUTTON: &str =
    "px-2.5 py-1 rounded-md text-[12px] text-white/80 hover:bg-white/10 transition-colors";

/// Floating viewport controls: frame the model, toggle the ground grid, and
/// toggle the turntable spin. Grouped together as the viewport's "view" cluster.
#[component]
pub fn ViewControls(bridge: BridgeSlot, state: ViewerState) -> impl IntoView {
    let frame = move |_| {
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &ClientMessage::Frame);
        }
    };
    let toggle_grid = move |_| {
        let enabled = !state.grid.get_untracked();
        state.grid.set(enabled);
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &ClientMessage::SetGrid { enabled });
        }
    };
    let toggle_turntable = move |_| {
        let enabled = !state.turntable.get_untracked();
        state.turntable.set(enabled);
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &ClientMessage::SetTurntable { enabled });
        }
    };

    view! {
        <div class="fixed top-16 left-3 z-10 flex items-center gap-1 p-1 rounded-xl border border-white/10 bg-[#14161d]/85 backdrop-blur-md shadow-lg shadow-black/40">
            <button class=BUTTON on:click=frame>"Frame"</button>
            <button class=move || active(state.grid.get()) on:click=toggle_grid>
                "Grid"
            </button>
            <button class=move || active(state.turntable.get()) on:click=toggle_turntable>
                "Spin"
            </button>
        </div>
    }
}

fn active(on: bool) -> String {
    if on {
        format!("{BUTTON} bg-white/10 text-white")
    } else {
        BUTTON.to_string()
    }
}
