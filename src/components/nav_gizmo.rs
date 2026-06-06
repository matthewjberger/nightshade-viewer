use leptos::prelude::*;
use protocol::ClientMessage;

use crate::bridge::{Bridge, send};
use crate::state::ViewerState;

const RADIUS: f32 = 26.0;
const CENTER: f32 = 38.0;

/// World axis, label, color, and whether it is the positive (labeled) end.
const AXES: [([f32; 3], &str, &str, bool); 6] = [
    ([1.0, 0.0, 0.0], "X", "#f2555a", true),
    ([0.0, 1.0, 0.0], "Y", "#5bb463", true),
    ([0.0, 0.0, 1.0], "Z", "#5a8cf0", true),
    ([-1.0, 0.0, 0.0], "X", "#f2555a", false),
    ([0.0, -1.0, 0.0], "Y", "#5bb463", false),
    ([0.0, 0.0, -1.0], "Z", "#5a8cf0", false),
];

/// A DOM orientation gizmo. Reads the camera basis the worker streams, projects
/// the world axes to screen, and snaps the view when an axis is clicked. The
/// page positions it, so it stays clear of the panels at any viewport size.
#[component]
pub fn NavGizmo(
    bridge: StoredValue<Option<Bridge>, LocalStorage>,
    state: ViewerState,
) -> impl IntoView {
    let snap = move |axis: [f32; 3]| {
        if let Some(bridge) = bridge.get_value() {
            send(&bridge, &ClientMessage::SnapAxis { axis });
        }
    };

    view! {
        <div class="fixed top-16 right-[19rem] z-10 select-none">
            <svg width="76" height="76" viewBox="0 0 76 76">
                {move || axes(state.camera_basis.get(), snap)}
            </svg>
        </div>
    }
}

struct Projected {
    depth: f32,
    x: f32,
    y: f32,
    label: &'static str,
    color: &'static str,
    positive: bool,
    direction: [f32; 3],
    opacity: f32,
}

fn axes(basis: [[f32; 3]; 3], snap: impl Fn([f32; 3]) + Copy + 'static) -> impl IntoView {
    let (right, up, forward) = (basis[0], basis[1], basis[2]);
    let mut items: Vec<Projected> = AXES
        .iter()
        .map(|(direction, label, color, positive)| {
            let depth = dot(*direction, forward);
            Projected {
                depth,
                x: CENTER + dot(*direction, right) * RADIUS,
                y: CENTER - dot(*direction, up) * RADIUS,
                label,
                color,
                positive: *positive,
                direction: *direction,
                opacity: (1.0 - 0.28 * (depth + 1.0)).clamp(0.4, 1.0),
            }
        })
        .collect();
    items.sort_by(|a, b| {
        b.depth
            .partial_cmp(&a.depth)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    items
        .into_iter()
        .map(
            |Projected {
                 x,
                 y,
                 label,
                 color,
                 positive,
                 direction,
                 opacity,
                 ..
             }| {
                view! {
                    {positive
                        .then(|| {
                            view! {
                                <line
                                    x1=CENTER
                                    y1=CENTER
                                    x2=x
                                    y2=y
                                    stroke=color
                                    stroke-width="2"
                                    opacity=opacity
                                />
                            }
                        })}
                    <circle
                        cx=x
                        cy=y
                        r=if positive { 9.0 } else { 6.0 }
                        fill=if positive { color } else { "#14161d" }
                        stroke=color
                        stroke-width=if positive { 0.0 } else { 1.5 }
                        opacity=opacity
                        class="cursor-pointer"
                        on:click=move |_| snap(direction)
                    />
                    {positive
                        .then(|| {
                            view! {
                                <text
                                    x=x
                                    y=y + 3.0
                                    text-anchor="middle"
                                    font-size="9"
                                    font-weight="600"
                                    fill="#ffffff"
                                    opacity=opacity
                                    class="pointer-events-none"
                                >
                                    {label}
                                </text>
                            }
                        })}
                }
            },
        )
        .collect_view()
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
