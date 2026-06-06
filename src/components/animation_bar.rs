use leptos::prelude::*;
use protocol::{ClientMessage, ClipInfo};

use crate::bridge::{Bridge, send};
use crate::state::ViewerState;

type BridgeSlot = StoredValue<Option<Bridge>, LocalStorage>;

/// A media-player-style transport for the loaded model's animation clips.
#[component]
pub fn AnimationBar(bridge: BridgeSlot, state: ViewerState) -> impl IntoView {
    let toggle_play = move |_| {
        if let Some(bridge) = bridge.get_value() {
            if state.anim_playing.get_untracked() {
                send(&bridge, &ClientMessage::PauseAnimation);
            } else if state.anim_clip.get_untracked().is_none() {
                send(&bridge, &ClientMessage::PlayAnimation { index: 0 });
            } else {
                send(&bridge, &ClientMessage::ResumeAnimation);
            }
        }
    };
    let stop = move |_| {
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &ClientMessage::StopAnimation);
        }
    };
    let on_seek = move |event| {
        if let Ok(time) = event_target_value(&event).parse::<f32>()
            && let Some(bridge) = bridge.get_value()
        {
            send(&bridge, &ClientMessage::SeekAnimation { time });
        }
    };
    let pick_clip = move |event| {
        if let Ok(index) = event_target_value(&event).parse::<u32>()
            && let Some(bridge) = bridge.get_value()
        {
            send(&bridge, &ClientMessage::PlayAnimation { index });
        }
    };
    let set_speed = move |event| {
        if let Ok(speed) = event_target_value(&event).parse::<f32>() {
            state.anim_speed.set(speed);
            if let Some(bridge) = bridge.get_value() {
                send(&bridge, &ClientMessage::SetAnimationSpeed { speed });
            }
        }
    };
    let toggle_loop = move |_| {
        let value = !state.anim_loop.get_untracked();
        state.anim_loop.set(value);
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &ClientMessage::SetAnimationLoop { looping: value });
        }
    };

    view! {
        <Show when=move || !state.clips.get().is_empty() fallback=|| ()>
            <div class="fixed bottom-4 left-1/2 -translate-x-1/2 z-10 flex items-center gap-3 px-4 py-2 rounded-xl border border-white/10 bg-[#14161d]/90 backdrop-blur-md shadow-lg shadow-black/40 w-[34rem] max-w-[92vw]">
                <button class="text-[14px] text-white/85 hover:text-white w-5" on:click=toggle_play>
                    {move || if state.anim_playing.get() { "❚❚" } else { "▶" }}
                </button>
                <button class="text-[13px] text-white/55 hover:text-white" on:click=stop>
                    "■"
                </button>
                <select
                    class="rounded-md bg-black/40 border border-white/10 px-2 py-1 text-[12px] text-white/90 outline-none focus:border-orange-400/60 max-w-[8rem]"
                    prop:value=move || {
                        state.anim_clip.get().map(|index| index.to_string()).unwrap_or_default()
                    }
                    on:change=pick_clip
                >
                    <For each=move || clip_rows(state) key=|(index, _)| *index let:item>
                        <option value=item.0.to_string()>{item.1.name.clone()}</option>
                    </For>
                </select>
                <input
                    type="range"
                    min="0"
                    max=move || state.anim_duration.get().to_string()
                    step="0.01"
                    prop:value=move || state.anim_time.get().to_string()
                    on:input=on_seek
                    class="flex-1 accent-orange-400"
                />
                <span class="text-[11px] text-white/45 tabular-nums w-20 text-right">
                    {move || {
                        format!("{:.1} / {:.1}s", state.anim_time.get(), state.anim_duration.get())
                    }}
                </span>
                <input
                    type="number"
                    step="0.1"
                    prop:value=move || format!("{:.1}", state.anim_speed.get())
                    on:input=set_speed
                    class="w-12 rounded-md bg-black/40 border border-white/10 px-1.5 py-1 text-[12px] text-white/90 outline-none tabular-nums"
                />
                <button
                    class=move || {
                        if state.anim_loop.get() {
                            "text-[14px] text-orange-300"
                        } else {
                            "text-[14px] text-white/40 hover:text-white/70"
                        }
                    }
                    on:click=toggle_loop
                >
                    "⟳"
                </button>
            </div>
        </Show>
    }
}

fn clip_rows(state: ViewerState) -> Vec<(usize, ClipInfo)> {
    state.clips.get().into_iter().enumerate().collect()
}
