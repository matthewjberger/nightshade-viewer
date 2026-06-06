use leptos::prelude::*;
use protocol::ClientMessage;
use web_sys::MouseEvent;

use crate::bridge::{Bridge, send};
use crate::state::{Browser, ViewerState};

/// A modal grid of fetchable assets: Khronos sample models or Polyhaven HDRIs.
#[component]
pub fn AssetBrowser(
    bridge: StoredValue<Option<Bridge>, LocalStorage>,
    state: ViewerState,
) -> impl IntoView {
    let close = move |_| state.browser.set(Browser::Closed);

    view! {
        <Show when=move || state.browser.get() != Browser::Closed fallback=|| ()>
            <div
                class="fixed inset-0 z-20 flex items-center justify-center bg-black/60 backdrop-blur-sm p-6 sm:p-10"
                on:click=close
            >
                <div
                    class="w-full max-w-4xl max-h-[82vh] flex flex-col rounded-2xl border border-white/10 bg-[#111319] shadow-2xl shadow-black/60 overflow-hidden"
                    on:click=move |event: MouseEvent| event.stop_propagation()
                >
                    <div class="flex items-center gap-2 px-4 py-3 border-b border-white/10">
                        <span class="text-[14px] font-semibold text-white/90">
                            {move || title(state.browser.get())}
                        </span>
                        <span class="text-[12px] text-white/40">
                            {move || subtitle(state.browser.get())}
                        </span>
                        <div class="flex-1"></div>
                        <button
                            class="px-2.5 py-1 rounded-md text-[12px] text-white/70 hover:bg-white/10"
                            on:click=close
                        >
                            "Close"
                        </button>
                    </div>
                    <div class="flex-1 overflow-y-auto p-4">{move || grid(bridge, state)}</div>
                </div>
            </div>
        </Show>
    }
}

fn title(browser: Browser) -> &'static str {
    match browser {
        Browser::Khronos => "Khronos Sample Assets",
        Browser::Polyhaven => "Polyhaven Environments",
        Browser::Closed => "",
    }
}

fn subtitle(browser: Browser) -> &'static str {
    match browser {
        Browser::Khronos => "glTF sample models",
        Browser::Polyhaven => "HDRI lighting",
        Browser::Closed => "",
    }
}

fn grid(bridge: StoredValue<Option<Bridge>, LocalStorage>, state: ViewerState) -> AnyView {
    match state.browser.get() {
        Browser::Khronos => khronos_grid(bridge, state).into_any(),
        Browser::Polyhaven => polyhaven_grid(bridge, state).into_any(),
        Browser::Closed => ().into_any(),
    }
}

fn khronos_grid(
    bridge: StoredValue<Option<Bridge>, LocalStorage>,
    state: ViewerState,
) -> impl IntoView {
    let load = move |name: String| {
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &ClientMessage::LoadKhronos { name });
        }
        state.browser.set(Browser::Closed);
    };

    view! {
        <div class="grid grid-cols-3 sm:grid-cols-4 gap-3">
            <For each=move || state.khronos.get() key=|entry| entry.name.clone() let:entry>
                {
                    let name = entry.name.clone();
                    let label = entry.label.clone();
                    let thumbnail = entry.thumbnail.clone();
                    view! {
                        <button
                            class="group rounded-lg overflow-hidden border border-white/10 bg-black/40 hover:border-orange-400/60 text-left transition-colors"
                            on:click=move |_| load(name.clone())
                        >
                            <div class="aspect-square overflow-hidden bg-black/50">
                                {thumbnail
                                    .map(|url| {
                                        view! {
                                            <img
                                                src=url
                                                loading="lazy"
                                                class="w-full h-full object-cover transition-transform group-hover:scale-105"
                                            />
                                        }
                                    })}
                            </div>
                            <div class="px-2 py-1.5 text-[12px] text-white/80 truncate">{label}</div>
                        </button>
                    }
                }
            </For>
            <Show when=move || state.khronos.get().is_empty() fallback=|| ()>
                <div class="col-span-full py-10 text-center text-[13px] text-white/40">
                    "Loading sample models…"
                </div>
            </Show>
        </div>
    }
}

fn polyhaven_grid(
    bridge: StoredValue<Option<Bridge>, LocalStorage>,
    state: ViewerState,
) -> impl IntoView {
    let load = move |slug: String| {
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &ClientMessage::LoadPolyhaven { slug });
        }
        state.browser.set(Browser::Closed);
    };

    view! {
        <div class="grid grid-cols-3 sm:grid-cols-4 gap-3">
            <For each=move || state.polyhaven.get() key=|entry| entry.slug.clone() let:entry>
                {
                    let slug = entry.slug.clone();
                    let name = entry.name.clone();
                    let thumbnail = entry.thumbnail.clone();
                    view! {
                        <button
                            class="group rounded-lg overflow-hidden border border-white/10 bg-black/40 hover:border-orange-400/60 text-left transition-colors"
                            on:click=move |_| load(slug.clone())
                        >
                            <div class="aspect-video overflow-hidden bg-black/50">
                                <img
                                    src=thumbnail
                                    loading="lazy"
                                    class="w-full h-full object-cover transition-transform group-hover:scale-105"
                                />
                            </div>
                            <div class="px-2 py-1.5 text-[12px] text-white/80 truncate">{name}</div>
                        </button>
                    }
                }
            </For>
            <Show when=move || state.polyhaven.get().is_empty() fallback=|| ()>
                <div class="col-span-full py-10 text-center text-[13px] text-white/40">
                    "Loading environments…"
                </div>
            </Show>
        </div>
    }
}
