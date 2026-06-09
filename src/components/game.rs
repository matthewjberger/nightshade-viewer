use leptos::prelude::*;
use protocol::{ClientMessage, GAME_LEVELS, GameCommand, GamePhase, game_level_name};

use crate::bridge::{Bridge, send};
use crate::state::{ViewerState, game_best, game_unlocked};

type BridgeSlot = StoredValue<Option<Bridge>, LocalStorage>;

const PANEL: &str = "rounded-2xl border border-white/10 bg-[#14161d]/95 backdrop-blur-md shadow-2xl shadow-black/60";
const ACTION: &str = "px-4 py-2 rounded-lg text-[13px] font-semibold transition-colors";

fn send_command(bridge: BridgeSlot, command: GameCommand) {
    if let Some(bridge) = bridge.get_value() {
        send(&bridge, &ClientMessage::Game { command });
    }
}

/// The siege game chrome: the start menu with level select, the in-game HUD
/// (score, combo, ammo, targets, hit popups), and the end-of-level overlays.
#[component]
pub fn Game(bridge: BridgeSlot, state: ViewerState) -> impl IntoView {
    view! {
        <GameMenu bridge state />
        <GameHud bridge state />
        <GameEnd bridge state />
    }
}

#[component]
fn GameMenu(bridge: BridgeSlot, state: ViewerState) -> impl IntoView {
    let open = move || state.game_menu_open.get() && state.game_phase.get() == GamePhase::Idle;
    let close = move |_| state.game_menu_open.set(false);
    let cards = move || {
        state.game_phase.track();
        let unlocked = game_unlocked();
        (1..=GAME_LEVELS)
            .map(|level| level_card(bridge, state, level, level <= unlocked))
            .collect_view()
    };

    view! {
        <Show when=open fallback=|| ()>
            <div class="fixed inset-0 z-30 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm">
                <div class=format!("{PANEL} w-full max-w-[560px] p-6 sm:p-8")>
                    <div class="flex items-start justify-between">
                        <div>
                            <h1 class="text-[22px] font-bold tracking-[0.2em] text-white">
                                "NIGHTSHADE SIEGE"
                            </h1>
                            <p class="mt-1 text-[12px] text-white/55">
                                "Knock every glowing target off its perch before the cannonballs run out."
                            </p>
                        </div>
                        <button
                            class="text-white/40 hover:text-white/90 text-[16px] leading-none pl-3"
                            title="Back to the viewer"
                            on:click=close
                        >
                            "✕"
                        </button>
                    </div>
                    <div class="mt-5 grid grid-cols-1 sm:grid-cols-2 gap-2">{cards}</div>
                    <div class="mt-5 flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-white/45">
                        <span>
                            <span class="text-white/75">"Drag"</span>
                            " orbits the arena"
                        </span>
                        <span>
                            <span class="text-white/75">"Scroll"</span>
                            " zooms"
                        </span>
                        <span>
                            <span class="text-white/75">"Click"</span>
                            " fires a cannonball"
                        </span>
                        <span>
                            "Chain knockouts for combo multipliers"
                        </span>
                    </div>
                </div>
            </div>
        </Show>
    }
}

fn level_card(bridge: BridgeSlot, state: ViewerState, level: u32, unlocked: bool) -> impl IntoView {
    let best = game_best(level);
    let start = move |_| {
        if unlocked {
            state.game_menu_open.set(false);
            send_command(bridge, GameCommand::Start { level });
        }
    };
    let class = if unlocked {
        "group flex items-center gap-3 px-3 py-2.5 rounded-xl border border-white/10 bg-white/[0.03] hover:bg-violet-500/15 hover:border-violet-400/40 transition-colors text-left"
    } else {
        "flex items-center gap-3 px-3 py-2.5 rounded-xl border border-white/5 bg-white/[0.02] opacity-45 cursor-not-allowed text-left"
    };

    view! {
        <button class=class disabled=!unlocked on:click=start>
            <span class="shrink-0 w-8 h-8 flex items-center justify-center rounded-lg bg-violet-500/20 text-violet-300 text-[13px] font-bold">
                {level}
            </span>
            <span class="min-w-0 flex-1">
                <span class="block truncate text-[13px] text-white/90">{game_level_name(level)}</span>
                <span class="block text-[11px] text-white/45">
                    {if !unlocked {
                        "Clear the previous level to unlock".to_string()
                    } else if best > 0 {
                        format!("Best: {best}")
                    } else {
                        "Not yet cleared".to_string()
                    }}
                </span>
            </span>
            <Show when=move || !unlocked fallback=|| ()>
                <span class="shrink-0 text-white/35">"🔒"</span>
            </Show>
        </button>
    }
}

#[component]
fn GameHud(bridge: BridgeSlot, state: ViewerState) -> impl IntoView {
    let playing = move || state.game_phase.get() == GamePhase::Playing;
    let title = move || {
        format!(
            "Level {} · {}",
            state.game_level.get(),
            game_level_name(state.game_level.get())
        )
    };
    let score = move || format!("{}", state.game_score.get());
    let targets = move || {
        format!(
            "{} / {} targets",
            state.game_targets_total.get() - state.game_targets_left.get(),
            state.game_targets_total.get()
        )
    };
    let pips = move || {
        let total = state.game_shots_total.get();
        let left = state.game_shots_left.get();
        (0..total)
            .map(|index| {
                let class = if index < left {
                    "w-2.5 h-2.5 rounded-full bg-violet-400 shadow shadow-violet-500/50"
                } else {
                    "w-2.5 h-2.5 rounded-full bg-white/15"
                };
                view! { <span class=class></span> }
            })
            .collect_view()
    };
    let restart = move |_| {
        send_command(
            bridge,
            GameCommand::Start {
                level: state.game_level.get_untracked(),
            },
        );
    };
    let quit = move |_| {
        send_command(bridge, GameCommand::Exit);
        state.game_menu_open.set(true);
    };

    view! {
        <Show when=playing fallback=|| ()>
            <div class="fixed top-3 left-1/2 -translate-x-1/2 z-20 flex items-center gap-3 px-4 py-2 rounded-xl border border-white/10 bg-[#14161d]/85 backdrop-blur-md shadow-lg shadow-black/40 pointer-events-none">
                <span class="text-[12px] text-white/60 whitespace-nowrap">{title}</span>
                <span class="w-px h-4 bg-white/10"></span>
                <span class="text-[15px] font-bold text-white tabular-nums">{score}</span>
                <Show when=move || { state.game_combo.get() > 1 } fallback=|| ()>
                    <span class="px-1.5 py-0.5 rounded-md bg-orange-500/25 text-orange-300 text-[11px] font-bold animate-pulse">
                        {move || format!("x{}", state.game_combo.get())}
                    </span>
                </Show>
            </div>
            <div class="fixed top-3 right-3 z-20 flex items-center gap-1">
                <button
                    class="px-2.5 py-1 rounded-md text-[12px] text-white/80 hover:bg-white/10 border border-white/10 bg-[#14161d]/85 backdrop-blur-md transition-colors"
                    title="Rebuild this level"
                    on:click=restart
                >
                    "Restart"
                </button>
                <button
                    class="px-2.5 py-1 rounded-md text-[12px] text-white/80 hover:bg-white/10 border border-white/10 bg-[#14161d]/85 backdrop-blur-md transition-colors"
                    title="Back to the level menu"
                    on:click=quit
                >
                    "Quit"
                </button>
            </div>
            <div class="fixed bottom-4 left-1/2 -translate-x-1/2 z-20 flex flex-col items-center gap-1.5 pointer-events-none">
                <div class="flex items-center gap-3 px-4 py-2 rounded-xl border border-white/10 bg-[#14161d]/85 backdrop-blur-md shadow-lg shadow-black/40">
                    <div class="flex items-center gap-1">{pips}</div>
                    <span class="w-px h-4 bg-white/10"></span>
                    <span class="text-[12px] text-white/60 tabular-nums whitespace-nowrap">{targets}</span>
                </div>
                <span class="text-[11px] text-white/35">
                    "Drag to orbit · Scroll to zoom · Click to fire"
                </span>
            </div>
            <div class="fixed top-[38%] left-1/2 -translate-x-1/2 z-20 flex flex-col items-center gap-1 pointer-events-none">
                <For
                    each=move || state.game_hits.get()
                    key=|(id, _)| *id
                    children=|(_, text)| {
                        view! {
                            <span class="game-hit text-[22px] font-bold text-orange-300 drop-shadow-[0_2px_8px_rgba(249,115,22,0.6)]">
                                {text}
                            </span>
                        }
                    }
                />
            </div>
        </Show>
    }
}

#[component]
fn GameEnd(bridge: BridgeSlot, state: ViewerState) -> impl IntoView {
    let ended = move || {
        matches!(
            state.game_phase.get(),
            GamePhase::Cleared | GamePhase::Failed
        )
    };
    let cleared = move || state.game_phase.get() == GamePhase::Cleared;
    let stars = move || {
        let total = state.game_shots_total.get().max(1);
        let left = state.game_shots_left.get();
        let count = if left * 5 >= total * 2 {
            3
        } else if left * 5 >= total {
            2
        } else {
            1
        };
        (0..3_u32)
            .map(|index| {
                let class = if index < count {
                    "text-[28px] text-orange-300"
                } else {
                    "text-[28px] text-white/15"
                };
                view! { <span class=class>"★"</span> }
            })
            .collect_view()
    };
    let replay = move |_| {
        send_command(
            bridge,
            GameCommand::Start {
                level: state.game_level.get_untracked(),
            },
        );
    };
    let next = move |_| {
        send_command(
            bridge,
            GameCommand::Start {
                level: state.game_level.get_untracked() + 1,
            },
        );
    };
    let menu = move |_| {
        send_command(bridge, GameCommand::Exit);
        state.game_menu_open.set(true);
    };

    view! {
        <Show when=ended fallback=|| ()>
            <div class="fixed inset-0 z-30 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm">
                <div class=format!("{PANEL} w-full max-w-[400px] p-7 text-center")>
                    <Show
                        when=cleared
                        fallback=move || {
                            view! {
                                <h2 class="text-[20px] font-bold text-white">"Out of Cannonballs"</h2>
                                <p class="mt-2 text-[13px] text-white/55">
                                    {move || {
                                        format!(
                                            "{} of {} targets survived the siege.",
                                            state.game_targets_left.get(),
                                            state.game_targets_total.get(),
                                        )
                                    }}
                                </p>
                                <p class="mt-3 text-[15px] text-white/80 tabular-nums">
                                    {move || format!("Score: {}", state.game_score.get())}
                                </p>
                            }
                        }
                    >
                        <h2 class="text-[20px] font-bold text-white">"Level Cleared!"</h2>
                        <div class="mt-2 flex items-center justify-center gap-1">{stars}</div>
                        <p class="mt-3 text-[26px] font-bold text-white tabular-nums">
                            {move || state.game_score.get()}
                        </p>
                        <p class="mt-1 text-[12px] text-white/50 tabular-nums">
                            {move || {
                                format!(
                                    "{} cannonballs spared · Best: {}",
                                    state.game_shots_left.get(),
                                    game_best(state.game_level.get()),
                                )
                            }}
                        </p>
                    </Show>
                    <div class="mt-6 flex items-center justify-center gap-2">
                        <Show
                            when=move || cleared() && state.game_level.get() < GAME_LEVELS
                            fallback=|| ()
                        >
                            <button
                                class=format!(
                                    "{ACTION} bg-violet-500/85 hover:bg-violet-400 text-white"
                                )
                                on:click=next
                            >
                                "Next Level"
                            </button>
                        </Show>
                        <button
                            class=format!(
                                "{ACTION} bg-white/10 hover:bg-white/20 text-white/90"
                            )
                            on:click=replay
                        >
                            {move || if cleared() { "Replay" } else { "Retry" }}
                        </button>
                        <button
                            class=format!(
                                "{ACTION} bg-white/5 hover:bg-white/15 text-white/70"
                            )
                            on:click=menu
                        >
                            "Menu"
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
}
