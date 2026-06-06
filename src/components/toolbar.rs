use leptos::html;
use leptos::prelude::*;
use protocol::ClientMessage;
use web_sys::{Event, HtmlElement};

use crate::bridge::{Bridge, send, send_file};
use crate::state::{Browser, ViewerState};

const BUTTON: &str =
    "shrink-0 px-2.5 py-1 rounded-md text-[12px] text-white/80 hover:bg-white/10 transition-colors";

/// The top bar: open a local file, open an asset browser, frame the model, and
/// a live status readout.
#[component]
pub fn Toolbar(
    bridge: StoredValue<Option<Bridge>, LocalStorage>,
    state: ViewerState,
) -> impl IntoView {
    let file_ref = NodeRef::<html::Input>::new();

    let open_file = move |_| {
        if let Some(input) = file_ref.get() {
            let element: &HtmlElement = input.as_ref();
            element.click();
        }
    };
    let on_file = move |_event: Event| {
        if let (Some(input), Some(bridge)) = (file_ref.get(), bridge.get_value())
            && let Some(files) = input.files()
            && let Some(file) = files.item(0)
        {
            send_file(&bridge, file);
        }
    };
    let frame = move |_| {
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &ClientMessage::Frame);
        }
    };
    let random_model = move |_| {
        let list = state.khronos.get_untracked();
        if let Some(bridge) = bridge.get_value()
            && !list.is_empty()
        {
            let index = ((js_sys::Math::random() * list.len() as f64) as usize).min(list.len() - 1);
            send(
                &bridge,
                &ClientMessage::LoadKhronos {
                    name: list[index].name.clone(),
                },
            );
        }
    };
    let random_hdri = move |_| {
        let list = state.hdris.get_untracked();
        if let Some(bridge) = bridge.get_value()
            && !list.is_empty()
        {
            let index = ((js_sys::Math::random() * list.len() as f64) as usize).min(list.len() - 1);
            send(
                &bridge,
                &ClientMessage::LoadPolyhaven {
                    slug: list[index].slug.clone(),
                    resolution: 4,
                },
            );
        }
    };
    let toggle_grid = move |_| {
        let enabled = !state.grid.get_untracked();
        state.grid.set(enabled);
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &ClientMessage::SetGrid { enabled });
        }
    };
    let grid_class = move || {
        if state.grid.get() {
            format!("{BUTTON} bg-white/10 text-white")
        } else {
            BUTTON.to_string()
        }
    };
    let toggle_turntable = move |_| {
        let enabled = !state.turntable.get_untracked();
        state.turntable.set(enabled);
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &ClientMessage::SetTurntable { enabled });
        }
    };
    let turntable_class = move || {
        if state.turntable.get() {
            format!("{BUTTON} bg-white/10 text-white")
        } else {
            BUTTON.to_string()
        }
    };
    let status = move || match state.loading.get() {
        Some(label) => format!("Loading {label}…"),
        None => format!("{:.0} fps", state.fps.get()),
    };
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
    let menu_open = RwSignal::new(false);
    let toggle_menu = move |_| menu_open.update(|open| *open = !*open);
    let close_menu = move || menu_open.set(false);
    let actions_class = move || {
        let desktop = "sm:static sm:mt-0 sm:flex sm:flex-row sm:items-center sm:gap-1 sm:p-0 sm:rounded-none sm:border-0 sm:bg-transparent sm:shadow-none";
        if menu_open.get() {
            format!(
                "absolute top-full left-0 right-0 mt-2 z-10 flex flex-col items-stretch gap-1 p-2 rounded-xl border border-white/10 bg-[#14161d]/95 backdrop-blur-md shadow-lg shadow-black/40 {desktop}"
            )
        } else {
            format!("hidden {desktop}")
        }
    };

    view! {
        <div class="fixed top-3 left-3 right-3 h-10 z-10 flex items-center gap-1 px-3 rounded-xl border border-white/10 bg-[#14161d]/85 backdrop-blur-md shadow-lg shadow-black/40">
            <button
                class=format!("{BUTTON} sm:hidden text-[15px] leading-none")
                on:click=toggle_menu
            >
                "☰"
            </button>
            <span class="shrink min-w-0 truncate text-[13px] font-semibold text-white/90 mr-1 sm:mr-3">
                "Nightshade"
            </span>
            <div class=actions_class>
                <button
                    class=scene_class
                    on:click=move |event| {
                        close_menu();
                        toggle_scene(event);
                    }
                >
                    "Scene"
                </button>
                <button
                    class=inspector_class
                    on:click=move |event| {
                        close_menu();
                        toggle_inspector(event);
                    }
                >
                    "Inspect"
                </button>
                <button class=BUTTON on:click=move |event| { close_menu(); open_file(event) }>
                    "Open"
                </button>
                <button
                    class=BUTTON
                    on:click=move |_| {
                        close_menu();
                        state.browser.set(Browser::Khronos);
                    }
                >
                    "Browse"
                </button>
                <button
                    class=BUTTON
                    on:click=move |_| {
                        close_menu();
                        state.add_open.set(true);
                    }
                >
                    "Add"
                </button>
                <button class=BUTTON on:click=move |event| { close_menu(); frame(event) }>
                    "Frame"
                </button>
                <button class=grid_class on:click=move |event| { close_menu(); toggle_grid(event) }>
                    "Grid"
                </button>
                <button
                    class=turntable_class
                    on:click=move |event| {
                        close_menu();
                        toggle_turntable(event);
                    }
                >
                    "Spin"
                </button>
            </div>
            <div class="shrink-0 flex items-center gap-1">
                <span class="text-[11px] text-white/40 pl-1">"Random:"</span>
                <button class=BUTTON on:click=random_model>"Model"</button>
                <button class=BUTTON on:click=random_hdri>"Sky"</button>
            </div>
            <div class="hidden sm:block flex-1"></div>
            <span class="hidden sm:inline shrink-0 text-[11px] text-white/45 tabular-nums pl-1">
                {status}
            </span>
            <input
                type="file"
                node_ref=file_ref
                accept=".glb,.gltf,.hdr"
                class="hidden"
                on:change=on_file
            />
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
