use leptos::prelude::*;

use crate::state::ViewerState;

/// Loading indicators: a slim indeterminate bar while the renderer starts or an
/// asset loads, a centered card until the renderer is ready, and a small toast
/// while an asset downloads.
#[component]
pub fn Loader(state: ViewerState) -> impl IntoView {
    let bar_active = move || !state.ready.get() || state.loading.get().is_some();

    view! {
        <Show when=bar_active fallback=|| ()>
            <div class="fixed top-0 left-0 right-0 h-0.5 z-40 overflow-hidden bg-white/5">
                <div class="loading-bar h-full bg-orange-400"></div>
            </div>
        </Show>

        <Show when=move || !state.ready.get() fallback=|| ()>
            <div class="fixed inset-0 z-40 flex items-center justify-center bg-[#0c0d12]/70 backdrop-blur-sm pointer-events-none">
                <div class="flex items-center gap-3 px-5 py-3 rounded-xl border border-white/10 bg-[#14161d]/90 text-white/85 text-[13px] shadow-2xl">
                    <span class="h-4 w-4 rounded-full border-2 border-white/20 border-t-orange-400 animate-spin"></span>
                    "Starting the renderer…"
                </div>
            </div>
        </Show>

        <Show
            when=move || state.ready.get() && state.loading.get().is_some()
            fallback=|| ()
        >
            <div class=move || {
                let right = if state.inspector_open.get() {
                    "right-3 sm:right-[19rem]"
                } else {
                    "right-3"
                };
                format!(
                    "fixed bottom-4 {right} z-30 flex items-center gap-2 px-3 py-2 rounded-lg border border-white/10 bg-[#14161d]/90 text-white/80 text-[12px] shadow-lg shadow-black/40"
                )
            }>
                <span class="h-3.5 w-3.5 rounded-full border-2 border-white/20 border-t-orange-400 animate-spin"></span>
                {move || {
                    state
                        .loading
                        .get()
                        .map(|label| format!("Loading {label}…"))
                        .unwrap_or_default()
                }}
            </div>
        </Show>
    }
}
