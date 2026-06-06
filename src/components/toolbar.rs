use leptos::html;
use leptos::prelude::*;
use protocol::ClientMessage;
use web_sys::{Event, HtmlElement};

use crate::bridge::{Bridge, send, send_file};
use crate::state::{Browser, ViewerState};

const BUTTON: &str =
    "px-2.5 py-1 rounded-md text-[12px] text-white/80 hover:bg-white/10 transition-colors";

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
    let status = move || match state.loading.get() {
        Some(label) => format!("Loading {label}…"),
        None => format!("{:.0} fps", state.fps.get()),
    };

    view! {
        <div class="fixed top-3 left-3 right-3 h-10 z-10 flex items-center gap-1 px-3 rounded-xl border border-white/10 bg-[#14161d]/85 backdrop-blur-md shadow-lg shadow-black/40">
            <span class="text-[13px] font-semibold text-white/90 mr-3">"Nightshade Viewer"</span>
            <button class=BUTTON on:click=open_file>"Open"</button>
            <button class=BUTTON on:click=move |_| state.browser.set(Browser::Khronos)>
                "Khronos"
            </button>
            <button class=BUTTON on:click=move |_| state.browser.set(Browser::Hdris)>
                "HDRIs"
            </button>
            <button class=BUTTON on:click=move |_| state.browser.set(Browser::Models)>
                "Models"
            </button>
            <button class=BUTTON on:click=frame>"Frame"</button>
            <div class="flex-1"></div>
            <span class="text-[11px] text-white/45 tabular-nums">{status}</span>
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
