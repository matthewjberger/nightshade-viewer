use leptos::prelude::*;
use protocol::ClientMessage;

use crate::bridge::{Bridge, send};
use crate::state::ViewerState;

/// The left panel: a flattened, indented hierarchy of the loaded model. Click a
/// row to select it (drives the inspector and the 3D outline).
#[component]
pub fn SceneTree(
    bridge: StoredValue<Option<Bridge>, LocalStorage>,
    state: ViewerState,
) -> impl IntoView {
    let select = move |id: u32| {
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &ClientMessage::Select { id });
        }
    };

    view! {
        <div class="fixed top-16 left-3 bottom-3 w-64 flex flex-col rounded-xl border border-white/10 bg-[#14161d]/85 backdrop-blur-md shadow-2xl shadow-black/40 overflow-hidden">
            <div class="px-3 py-2.5 text-[11px] font-semibold uppercase tracking-wider text-white/50 border-b border-white/10">
                "Scene"
            </div>
            <div class="flex-1 overflow-y-auto py-1 text-[13px]">
                <For each=move || state.scene.get() key=|node| node.id let:node>
                    {
                        let id = node.id;
                        let name = node.name.clone();
                        let depth = node.depth as usize;
                        let has_mesh = node.has_mesh;
                        let selected = move || {
                            state.selected.get().map(|detail| detail.id) == Some(id)
                        };
                        view! {
                            <button
                                class=move || {
                                    let base = "group w-full flex items-center gap-2 pr-3 py-1 text-left transition-colors";
                                    if selected() {
                                        format!("{base} bg-orange-500/20 text-orange-200")
                                    } else {
                                        format!("{base} text-white/75 hover:bg-white/5")
                                    }
                                }
                                style=move || format!("padding-left: {}px", 10 + depth * 14)
                                on:click=move |_| select(id)
                            >
                                <span class=move || {
                                    if has_mesh {
                                        "h-1.5 w-1.5 rounded-sm bg-sky-400/80 shrink-0"
                                    } else {
                                        "h-1.5 w-1.5 rounded-sm bg-white/20 shrink-0"
                                    }
                                }></span>
                                <span class="truncate">{name}</span>
                            </button>
                        }
                    }
                </For>
                <Show when=move || state.scene.get().is_empty() fallback=|| ()>
                    <div class="px-3 py-4 text-[12px] text-white/35">"No model loaded."</div>
                </Show>
            </div>
        </div>
    }
}
