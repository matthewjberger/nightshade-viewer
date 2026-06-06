use leptos::prelude::*;

use crate::state::{ViewerState, mark_hint_seen};

/// A one-time, dismissable tip nudging first-time visitors toward the Browse and
/// Random shortcuts. Hidden for good once dismissed or once a shortcut is used.
#[component]
pub fn Hint(state: ViewerState) -> impl IntoView {
    let dismiss = move |_| {
        state.hint_open.set(false);
        mark_hint_seen();
    };

    view! {
        <Show when=move || state.hint_open.get() fallback=|| ()>
            <div class="fixed top-14 left-1/2 -translate-x-1/2 z-20 flex items-center gap-2 px-3 py-1.5 rounded-full border border-white/10 bg-[#14161d]/95 backdrop-blur-md shadow-lg shadow-black/40 text-[12px] text-white/75">
                <span>
                    "Tip: try " <span class="text-white">"Random"</span> " or "
                    <span class="text-white">"Browse"</span> " to load a model"
                </span>
                <button
                    class="text-white/40 hover:text-white/80 leading-none"
                    title="Dismiss"
                    on:click=dismiss
                >
                    "✕"
                </button>
            </div>
        </Show>
    }
}
