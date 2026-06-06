use leptos::prelude::*;
use protocol::{ClientMessage, PbrDebug, ShadingMode, Tonemap};

use crate::bridge::{Bridge, send};
use crate::state::{PanelTab, Validation, ViewerState};

type BridgeSlot = StoredValue<Option<Bridge>, LocalStorage>;

const SHADING: [(&str, ShadingMode); 4] = [
    ("Rendered", ShadingMode::Rendered),
    ("Solid", ShadingMode::Solid),
    ("Flat", ShadingMode::Flat),
    ("Wireframe", ShadingMode::Wireframe),
];
const PBR: [(&str, PbrDebug); 7] = [
    ("Off", PbrDebug::Off),
    ("Base color", PbrDebug::BaseColor),
    ("Normal", PbrDebug::Normal),
    ("Metallic", PbrDebug::Metallic),
    ("Roughness", PbrDebug::Roughness),
    ("Occlusion", PbrDebug::Occlusion),
    ("Emissive", PbrDebug::Emissive),
];
const TONEMAP: [(&str, Tonemap); 6] = [
    ("ACES", Tonemap::Aces),
    ("Reinhard", Tonemap::Reinhard),
    ("Uncharted 2", Tonemap::Uncharted2),
    ("AgX", Tonemap::AgX),
    ("Neutral", Tonemap::Neutral),
    ("None", Tonemap::None),
];

/// The left panel: a tabbed Scene tree, render settings, and model stats.
#[component]
pub fn LeftPanel(bridge: BridgeSlot, state: ViewerState) -> impl IntoView {
    let tab_class = move |tab: PanelTab| {
        let base =
            "flex-1 px-2 py-2 text-[11px] font-semibold uppercase tracking-wider transition-colors";
        if state.tab.get() == tab {
            format!("{base} text-white bg-white/10")
        } else {
            format!("{base} text-white/45 hover:text-white/80")
        }
    };

    let panel_class = move || {
        let base = "fixed top-16 left-3 bottom-3 w-72 max-w-[85vw] z-20 flex flex-col rounded-xl border border-white/10 bg-[#14161d]/85 backdrop-blur-md shadow-2xl shadow-black/40 overflow-hidden transition-transform duration-200";
        if state.scene_open.get() {
            format!("{base} translate-x-0")
        } else {
            format!("{base} -translate-x-[120%]")
        }
    };

    view! {
        <Show when=move || state.scene_open.get() fallback=|| ()>
            <div
                class="fixed inset-0 z-10 bg-black/40 sm:hidden"
                on:click=move |_| state.scene_open.set(false)
            ></div>
        </Show>
        <div class=panel_class>
            <div class="flex border-b border-white/10">
                <button
                    class=move || tab_class(PanelTab::Scene)
                    on:click=move |_| state.tab.set(PanelTab::Scene)
                >
                    "Scene"
                </button>
                <button
                    class=move || tab_class(PanelTab::Render)
                    on:click=move |_| state.tab.set(PanelTab::Render)
                >
                    "Render"
                </button>
                <button
                    class=move || tab_class(PanelTab::Stats)
                    on:click=move |_| state.tab.set(PanelTab::Stats)
                >
                    "Stats"
                </button>
            </div>
            <div class="flex-1 overflow-y-auto">
                {move || match state.tab.get() {
                    PanelTab::Scene => scene_tab(bridge, state).into_any(),
                    PanelTab::Render => render_tab(bridge, state).into_any(),
                    PanelTab::Stats => stats_tab(state).into_any(),
                }}
            </div>
        </div>
    }
}

fn scene_tab(bridge: BridgeSlot, state: ViewerState) -> impl IntoView {
    let select = move |id: u32| {
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &ClientMessage::Select { id });
        }
        state.scene_open.set(false);
    };
    view! {
        <div class="py-1 text-[13px]">
            <For each=move || state.scene.get() key=|node| node.id let:node>
                {
                    let id = node.id;
                    let name = node.name.clone();
                    let depth = node.depth as usize;
                    let has_mesh = node.has_mesh;
                    let selected = move || state.selected.get().map(|detail| detail.id) == Some(id);
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
                            <span class=if has_mesh {
                                "h-1.5 w-1.5 rounded-sm bg-sky-400/80 shrink-0"
                            } else {
                                "h-1.5 w-1.5 rounded-sm bg-white/20 shrink-0"
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
    }
}

fn render_tab(bridge: BridgeSlot, state: ViewerState) -> impl IntoView {
    view! {
        <div class="p-3 space-y-4 text-[12px]">
            <div class="space-y-2">
                <div class="text-[11px] uppercase tracking-wider text-white/40">"Shading"</div>
                {enum_select(
                    &SHADING,
                    state.shading,
                    move |mode| sendc(bridge, ClientMessage::SetShadingMode { mode }),
                )}
                {labeled(
                    "Debug",
                    enum_select(
                        &PBR,
                        state.pbr_debug,
                        move |mode| sendc(bridge, ClientMessage::SetPbrDebug { mode }),
                    ),
                )}
                {toggle(
                    "Normals",
                    state.show_normals,
                    move |enabled| sendc(bridge, ClientMessage::SetShowNormals { enabled }),
                )}
                {toggle(
                    "Bounds",
                    state.show_bounds,
                    move |enabled| sendc(bridge, ClientMessage::SetShowBounds { enabled }),
                )}
            </div>
            <div class="space-y-2">
                <div class="text-[11px] uppercase tracking-wider text-white/40">"Environment"</div>
                {toggle(
                    "Show sky",
                    state.show_sky,
                    move |show| sendc(bridge, ClientMessage::SetShowSky { show }),
                )}
                {slider(
                    "Exposure",
                    state.exposure,
                    0.1,
                    5.0,
                    0.01,
                    move |exposure| sendc(bridge, ClientMessage::SetExposure { exposure }),
                )}
                {labeled(
                    "Tone map",
                    enum_select(
                        &TONEMAP,
                        state.tonemap,
                        move |algorithm| sendc(bridge, ClientMessage::SetTonemap { algorithm }),
                    ),
                )}
            </div>
            {variant_section(bridge, state)}
        </div>
    }
}

fn variant_section(bridge: BridgeSlot, state: ViewerState) -> impl IntoView {
    view! {
        <Show when=move || !state.variants.get().is_empty() fallback=|| ()>
            <div class="space-y-2">
                <div class="text-[11px] uppercase tracking-wider text-white/40">"Material variant"</div>
                <select
                    class="w-full rounded-md bg-black/40 border border-white/10 px-2 py-1 text-white/90 outline-none focus:border-orange-400/60"
                    prop:value=move || state.active_variant.get().unwrap_or_default()
                    on:change=move |event| {
                        let value = event_target_value(&event);
                        let name = if value.is_empty() { None } else { Some(value) };
                        state.active_variant.set(name.clone());
                        if let Some(bridge) = bridge.get_value() {
                            send(&bridge, &ClientMessage::SetVariant { name });
                        }
                    }
                >
                    <option value="">"Default"</option>
                    <For each=move || state.variants.get() key=|name| name.clone() let:name>
                        <option value=name.clone()>{name.clone()}</option>
                    </For>
                </select>
            </div>
        </Show>
    }
}

fn stats_tab(state: ViewerState) -> impl IntoView {
    view! {
        <div class="p-3 text-[12px] space-y-3">
            <Show
                when=move || state.stats.get().is_some()
                fallback=|| view! { <div class="text-white/35">"No model loaded."</div> }
            >
                {move || {
                    let stats = state.stats.get().unwrap();
                    view! {
                        <div class="space-y-1.5">
                            {stat_row("Meshes", stats.meshes.to_string())}
                            {stat_row("Vertices", group(stats.vertices))}
                            {stat_row("Triangles", group(stats.triangles))}
                            {stat_row("Materials", stats.materials.to_string())}
                            {stat_row("Textures", stats.textures.to_string())}
                            {stat_row(
                                "Size",
                                format!(
                                    "{:.2} × {:.2} × {:.2}",
                                    stats.dimensions[0],
                                    stats.dimensions[1],
                                    stats.dimensions[2],
                                ),
                            )}
                        </div>
                    }
                }}
            </Show>
            {move || state.validation.get().map(validation_row)}
        </div>
    }
}

fn validation_row(validation: Validation) -> impl IntoView {
    let (text, color) = if validation.errors > 0 {
        (
            format!(
                "{} error{}, {} warning{}",
                validation.errors,
                plural(validation.errors),
                validation.warnings,
                plural(validation.warnings),
            ),
            "text-red-300",
        )
    } else if validation.warnings > 0 {
        (
            format!(
                "{} warning{}",
                validation.warnings,
                plural(validation.warnings)
            ),
            "text-amber-300",
        )
    } else {
        ("Valid glTF".to_string(), "text-emerald-300")
    };
    view! {
        <div class=format!(
            "pt-2 border-t border-white/10 {color}",
        )>{format!("Validator: {text}")}</div>
    }
}

fn plural(count: u32) -> &'static str {
    if count == 1 { "" } else { "s" }
}

fn sendc(bridge: BridgeSlot, message: ClientMessage) {
    if let Some(bridge) = bridge.get_value() {
        send(&bridge, &message);
    }
}

fn enum_select<T: Copy + PartialEq + Send + Sync + 'static>(
    options: &'static [(&'static str, T)],
    current: RwSignal<T>,
    on_pick: impl Fn(T) + Copy + 'static,
) -> impl IntoView {
    view! {
        <select
            class="w-full rounded-md bg-black/40 border border-white/10 px-2 py-1 text-white/90 outline-none focus:border-orange-400/60"
            prop:value=move || {
                options.iter().position(|(_, value)| *value == current.get()).unwrap_or(0).to_string()
            }
            on:change=move |event| {
                if let Ok(index) = event_target_value(&event).parse::<usize>()
                    && let Some((_, value)) = options.get(index)
                {
                    current.set(*value);
                    on_pick(*value);
                }
            }
        >
            {options
                .iter()
                .enumerate()
                .map(|(index, (label, _))| view! { <option value=index.to_string()>{*label}</option> })
                .collect_view()}
        </select>
    }
}

fn labeled(label: &'static str, control: impl IntoView + 'static) -> impl IntoView {
    view! {
        <div class="flex items-center gap-2">
            <span class="w-16 text-white/50 shrink-0">{label}</span>
            <div class="flex-1">{control}</div>
        </div>
    }
}

fn toggle(
    label: &'static str,
    signal: RwSignal<bool>,
    on_change: impl Fn(bool) + Copy + 'static,
) -> impl IntoView {
    view! {
        <button
            class="w-full flex items-center justify-between py-0.5 text-white/70 hover:text-white/90"
            on:click=move |_| {
                let value = !signal.get_untracked();
                signal.set(value);
                on_change(value);
            }
        >
            <span>{label}</span>
            <span class=move || {
                if signal.get() {
                    "h-3.5 w-3.5 rounded bg-orange-400"
                } else {
                    "h-3.5 w-3.5 rounded border border-white/20"
                }
            }></span>
        </button>
    }
}

fn slider(
    label: &'static str,
    signal: RwSignal<f32>,
    min: f32,
    max: f32,
    step: f32,
    on_change: impl Fn(f32) + Copy + 'static,
) -> impl IntoView {
    view! {
        <div class="flex items-center gap-2">
            <span class="w-16 text-white/50 shrink-0">{label}</span>
            <input
                type="range"
                min=min.to_string()
                max=max.to_string()
                step=step.to_string()
                prop:value=move || signal.get().to_string()
                on:input=move |event| {
                    if let Ok(value) = event_target_value(&event).parse::<f32>() {
                        signal.set(value);
                        on_change(value);
                    }
                }
                class="flex-1 accent-orange-400"
            />
            <span class="w-10 text-right text-white/60 tabular-nums">
                {move || format!("{:.2}", signal.get())}
            </span>
        </div>
    }
}

fn stat_row(label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="flex justify-between">
            <span class="text-white/45">{label}</span>
            <span class="text-white/85 tabular-nums">{value}</span>
        </div>
    }
}

fn group(value: u32) -> String {
    let digits = value.to_string();
    let bytes = digits.as_bytes();
    let mut out = String::new();
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 && (bytes.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*byte as char);
    }
    out
}
