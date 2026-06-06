use leptos::prelude::*;
use protocol::{ClientMessage, PolyhavenEntry};
use wasm_bindgen::JsCast;
use web_sys::{Event, MouseEvent};

use crate::bridge::{Bridge, send};
use crate::state::{Browser, ViewerState};

/// A modal, searchable grid of fetchable assets: Khronos sample models,
/// Polyhaven HDRIs, or Polyhaven models.
#[component]
pub fn AssetBrowser(
    bridge: StoredValue<Option<Bridge>, LocalStorage>,
    state: ViewerState,
) -> impl IntoView {
    let search = RwSignal::new(String::new());
    let close = move |_| {
        state.browser.set(Browser::Closed);
        search.set(String::new());
    };

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
                    <div class="flex items-center gap-3 px-4 py-3 border-b border-white/10">
                        <div class="flex flex-col">
                            <span class="text-[14px] font-semibold text-white/90 leading-tight">
                                {move || title(state.browser.get())}
                            </span>
                            <span class="text-[11px] text-white/40">
                                {move || subtitle(state.browser.get())}
                            </span>
                        </div>
                        <input
                            class="flex-1 max-w-xs ml-2 rounded-md bg-black/40 border border-white/10 px-2.5 py-1.5 text-[13px] text-white/90 placeholder:text-white/30 outline-none focus:border-orange-400/60"
                            type="text"
                            placeholder="Search…"
                            prop:value=move || search.get()
                            on:input=move |event| search.set(input_value(&event))
                        />
                        <div class="flex-1"></div>
                        <button
                            class="px-2.5 py-1 rounded-md text-[12px] text-white/70 hover:bg-white/10"
                            on:click=close
                        >
                            "Close"
                        </button>
                    </div>
                    <div class="flex-1 overflow-y-auto p-4">
                        {move || grid(bridge, state, search)}
                    </div>
                </div>
            </div>
        </Show>
    }
}

fn title(browser: Browser) -> &'static str {
    match browser {
        Browser::Khronos => "Khronos Sample Assets",
        Browser::Hdris => "Polyhaven Environments",
        Browser::Models => "Polyhaven Models",
        Browser::Closed => "",
    }
}

fn subtitle(browser: Browser) -> &'static str {
    match browser {
        Browser::Khronos => "glTF sample models",
        Browser::Hdris => "HDRI lighting",
        Browser::Models => "CC0 glTF models",
        Browser::Closed => "",
    }
}

fn grid(
    bridge: StoredValue<Option<Bridge>, LocalStorage>,
    state: ViewerState,
    search: RwSignal<String>,
) -> AnyView {
    match state.browser.get() {
        Browser::Khronos => khronos_grid(bridge, state, search).into_any(),
        Browser::Hdris => {
            poly_grid(bridge, state, state.hdris, search, false, "aspect-video").into_any()
        }
        Browser::Models => {
            poly_grid(bridge, state, state.models, search, true, "aspect-square").into_any()
        }
        Browser::Closed => ().into_any(),
    }
}

fn khronos_grid(
    bridge: StoredValue<Option<Bridge>, LocalStorage>,
    state: ViewerState,
    search: RwSignal<String>,
) -> impl IntoView {
    let load = move |name: String| {
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &ClientMessage::LoadKhronos { name });
        }
        state.browser.set(Browser::Closed);
    };
    let rows = move || {
        let query = search.get().to_lowercase();
        state
            .khronos
            .get()
            .into_iter()
            .filter(|entry| entry.label.to_lowercase().contains(&query))
            .collect::<Vec<_>>()
    };

    view! {
        <div class="grid grid-cols-3 sm:grid-cols-4 gap-3">
            <For each=rows key=|entry| entry.name.clone() let:entry>
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
            {move || empty_note(rows().is_empty())}
        </div>
    }
}

fn poly_grid(
    bridge: StoredValue<Option<Bridge>, LocalStorage>,
    state: ViewerState,
    entries: RwSignal<Vec<PolyhavenEntry>>,
    search: RwSignal<String>,
    is_model: bool,
    thumb_aspect: &'static str,
) -> impl IntoView {
    let load = move |slug: String| {
        if let Some(bridge) = bridge.get_value() {
            let message = if is_model {
                ClientMessage::LoadPolyhavenModel { slug }
            } else {
                ClientMessage::LoadPolyhaven { slug }
            };
            send(&bridge, &message);
        }
        state.browser.set(Browser::Closed);
    };
    let rows = move || {
        let query = search.get().to_lowercase();
        entries
            .get()
            .into_iter()
            .filter(|entry| entry.name.to_lowercase().contains(&query))
            .collect::<Vec<_>>()
    };

    view! {
        <div class="grid grid-cols-3 sm:grid-cols-4 gap-3">
            <For each=rows key=|entry| entry.slug.clone() let:entry>
                {
                    let slug = entry.slug.clone();
                    let name = entry.name.clone();
                    let thumbnail = entry.thumbnail.clone();
                    view! {
                        <button
                            class="group rounded-lg overflow-hidden border border-white/10 bg-black/40 hover:border-orange-400/60 text-left transition-colors"
                            on:click=move |_| load(slug.clone())
                        >
                            <div class=format!("{thumb_aspect} overflow-hidden bg-black/50")>
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
            {move || empty_note(rows().is_empty())}
        </div>
    }
}

fn empty_note(empty: bool) -> impl IntoView {
    empty.then(|| {
        view! {
            <div class="col-span-full py-10 text-center text-[13px] text-white/40">
                "Nothing to show yet."
            </div>
        }
    })
}

fn input_value(event: &Event) -> String {
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|input| input.value())
        .unwrap_or_default()
}
