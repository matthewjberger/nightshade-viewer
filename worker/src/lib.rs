mod agent;
mod ecs;
mod state;
mod systems;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use nightshade::prelude::winit::event::{
    ElementState as WinitElementState, MouseButton as WinitMouseButton,
};
use nightshade::prelude::*;
use nightshade::render::wgpu::create_wgpu_renderer;
use protocol::{
    AssetKind, BYTES_KEY, ClientMessage, GLTF_KEY, GizmoKind, MESSAGE_KEY, RESOURCES_KEY,
    WorkerMessage,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent, OffscreenCanvas};

use crate::ecs::PendingAsset;
use crate::state::Viewer;

type AppSlot = Rc<RefCell<Option<App>>>;
type AnimState = Rc<RefCell<Option<(f32, bool, Option<u32>)>>>;

struct App {
    world: World,
    renderer: WgpuRenderer,
    state: Viewer,
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();

    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let app_slot: AppSlot = Rc::new(RefCell::new(None));

    let handler_scope = scope.clone();
    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        handle_message(&handler_scope, &app_slot, event);
    });
    scope.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
}

fn handle_message(scope: &DedicatedWorkerGlobalScope, app_slot: &AppSlot, event: MessageEvent) {
    let data = event.data();
    let Ok(payload) = js_sys::Reflect::get(&data, &JsValue::from_str(MESSAGE_KEY)) else {
        return;
    };
    let Ok(message) = serde_wasm_bindgen::from_value::<ClientMessage>(payload) else {
        return;
    };

    match message {
        ClientMessage::Init { width, height } => {
            let Some(canvas) = canvas_from(&data) else {
                return;
            };
            let scope = scope.clone();
            let app_slot = app_slot.clone();
            spawn_local(async move {
                let app = create_app(canvas, width, height).await;
                post(&WorkerMessage::Ready {
                    context: context(),
                    adapter: "WebGPU".to_string(),
                });
                *app_slot.borrow_mut() = Some(app);
                start_render_loop(scope, app_slot);
            });
        }
        ClientMessage::Resize { width, height } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                let physical_width = (width as u32).max(1);
                let physical_height = (height as u32).max(1);
                resize_offscreen(
                    &mut app.world,
                    &mut app.renderer,
                    physical_width,
                    physical_height,
                );
                app.world.resources.window.active_viewport_rect =
                    Some(nightshade::ecs::window::resources::ViewportRect {
                        x: 0.0,
                        y: 0.0,
                        width: physical_width as f32,
                        height: physical_height as f32,
                    });
            }
        }
        ClientMessage::PointerMove { x, y } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                input_inject_cursor_moved(&mut app.world, Vec2::new(x, y));
            }
        }
        ClientMessage::PointerButton { button, pressed } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                let state = if pressed {
                    WinitElementState::Pressed
                } else {
                    WinitElementState::Released
                };
                input_inject_mouse_button(&mut app.world, mouse_button(button), state);
            }
        }
        ClientMessage::Wheel { delta } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                input_inject_mouse_wheel(&mut app.world, Vec2::new(0.0, -delta / 100.0));
            }
        }
        ClientMessage::Touch { id, phase, x, y } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                input_inject_touch(&mut app.world, id, touch_phase(phase), Vec2::new(x, y));
            }
        }
        ClientMessage::Pick { x, y } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::picking::request(&mut app.state.viewer, &mut app.world, x, y);
            }
        }
        ClientMessage::Select { id } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::selection::select_by_id(&mut app.state.viewer, &mut app.world, id);
            }
        }
        ClientMessage::Deselect => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::selection::select(&mut app.state.viewer, &mut app.world, None);
            }
        }
        ClientMessage::AddPrimitive { kind } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::spawn::add_primitive(&mut app.state.viewer, &mut app.world, kind);
            }
        }
        ClientMessage::AddLight { kind } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::spawn::add_light(&mut app.state.viewer, &mut app.world, kind);
            }
        }
        ClientMessage::SetTransform {
            id,
            translation,
            rotation,
            scale,
        } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::selection::set_transform(&mut app.world, id, translation, rotation, scale);
            }
        }
        ClientMessage::SetGizmoMode { mode } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                app.world.resources.user_interface.gizmos.mode = match mode {
                    GizmoKind::Translate => nightshade::ecs::gizmos::GizmoMode::LocalTranslation,
                    GizmoKind::Rotate => nightshade::ecs::gizmos::GizmoMode::Rotation,
                    GizmoKind::Scale => nightshade::ecs::gizmos::GizmoMode::Scale,
                };
            }
        }
        ClientMessage::SetGrid { enabled } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                app.world.resources.debug_draw.show_grid = enabled;
            }
        }
        ClientMessage::SnapAxis { axis } => {
            if let Some(app) = app_slot.borrow_mut().as_mut()
                && let Some(camera) = app.world.resources.active_camera
                && let Some(orbit) = app.world.core.get_pan_orbit_camera_mut(camera)
            {
                let direction = Vec3::new(axis[0], axis[1], axis[2]);
                let unit = if direction.norm() > 0.0001 {
                    direction.normalize()
                } else {
                    direction
                };
                orbit.target_yaw = unit.x.atan2(unit.z);
                orbit.target_pitch = unit.y.clamp(-1.0, 1.0).asin();
            }
        }
        ClientMessage::PlayAnimation { index } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                animate(app, |player| player.play(index as usize));
            }
        }
        ClientMessage::PauseAnimation => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                animate(app, |player| player.pause());
            }
        }
        ClientMessage::ResumeAnimation => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                animate(app, |player| player.resume());
            }
        }
        ClientMessage::StopAnimation => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                animate(app, |player| player.stop());
            }
        }
        ClientMessage::SeekAnimation { time } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                animate(app, |player| player.time = time.max(0.0));
            }
        }
        ClientMessage::SetAnimationSpeed { speed } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                animate(app, |player| player.speed = speed);
            }
        }
        ClientMessage::SetAnimationLoop { looping } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                animate(app, |player| player.looping = looping);
            }
        }
        ClientMessage::SetShadingMode { mode } => {
            if let Some(app) = app_slot.borrow_mut().as_mut()
                && let Some(camera) = app.world.resources.active_camera
            {
                app.world.core.set_viewport_shading(
                    camera,
                    nightshade::ecs::camera::components::ViewportShading {
                        mode: map_shading(mode),
                        show_overlays: true,
                    },
                );
            }
        }
        ClientMessage::SetPbrDebug { mode } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                app.world.resources.debug_draw.pbr_debug_mode = map_pbr(mode);
            }
        }
        ClientMessage::SetShowNormals { enabled } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                app.world.resources.debug_draw.show_normals = enabled;
            }
        }
        ClientMessage::SetShowBounds { enabled } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                app.world.resources.debug_draw.show_bounding_volumes = enabled;
            }
        }
        ClientMessage::SetExposure { exposure } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                app.world.resources.render_settings.color_grading.exposure = exposure;
            }
        }
        ClientMessage::SetTonemap { algorithm } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                app.world
                    .resources
                    .render_settings
                    .color_grading
                    .tonemap_algorithm = map_tonemap(algorithm);
            }
        }
        ClientMessage::SetShowSky { show } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                app.world.resources.render_settings.show_sky = show;
            }
        }
        ClientMessage::SetVariant { name } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                nightshade::ecs::material::commands::material_variant_apply(
                    &mut app.world,
                    name.as_deref(),
                );
            }
        }
        ClientMessage::Frame => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                app.state.viewer.resources.camera_input.frame_requested = true;
            }
        }
        ClientMessage::SetTurntable { enabled } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                app.state.viewer.resources.camera_input.turntable = enabled;
            }
        }
        ClientMessage::DropAsset { kind } => {
            if let Some(app) = app_slot.borrow_mut().as_mut()
                && let Some(bytes) = bytes_from(&data)
                && let Ok(mut slot) = app.state.viewer.resources.incoming.asset.lock()
            {
                *slot = Some(match kind {
                    AssetKind::Model => PendingAsset::Model(bytes),
                    AssetKind::Hdri => PendingAsset::Hdri(bytes),
                });
            }
        }
        ClientMessage::LoadGltfBundle => {
            if let Some(app) = app_slot.borrow_mut().as_mut()
                && let Some(gltf) = bytes_with_key(&data, GLTF_KEY)
                && let Ok(mut slot) = app.state.viewer.resources.incoming.asset.lock()
            {
                *slot = Some(PendingAsset::ModelWithResources {
                    gltf,
                    resources: resources_from(&data),
                });
            }
        }
        ClientMessage::LoadKhronos { name } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::browsers::fetch_khronos(&app.state.viewer, &name);
            }
        }
        ClientMessage::LoadPolyhaven { slug, resolution } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::browsers::fetch_polyhaven(&app.state.viewer, &slug, resolution);
            }
        }
        ClientMessage::LoadPolyhavenModel { slug, resolution } => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::browsers::fetch_polyhaven_model(&app.state.viewer, &slug, resolution);
            }
        }
        ClientMessage::RefreshBrowsers => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                systems::browsers::resend(&mut app.state.viewer);
            }
        }
        ClientMessage::Agent(request) => {
            if let Some(app) = app_slot.borrow_mut().as_mut() {
                agent::handle_agent_request(&mut app.world, &mut app.state, request);
            }
        }
    }
}

async fn create_app(canvas: OffscreenCanvas, width: f32, height: f32) -> App {
    let physical_width = (width as u32).max(1);
    let physical_height = (height as u32).max(1);

    let surface_target = wgpu::SurfaceTarget::OffscreenCanvas(canvas);
    let mut renderer = create_wgpu_renderer(surface_target, physical_width, physical_height)
        .await
        .expect("failed to create renderer from offscreen canvas");

    let mut world = World::default();
    let mut state = Viewer::default();
    initialize_offscreen(
        &mut world,
        &mut state,
        &mut renderer,
        (physical_width, physical_height),
        1.0,
    );
    agent::install_systems(&mut world);

    App {
        world,
        renderer,
        state,
    }
}

fn start_render_loop(_scope: DedicatedWorkerGlobalScope, app_slot: AppSlot) {
    let last_push = Rc::new(RefCell::new(0.0_f64));
    let last_basis = Rc::new(RefCell::new(None::<[[f32; 3]; 3]>));
    let last_anim: AnimState = Rc::new(RefCell::new(None));

    spawn_animation_frame_loop(move || {
        if let Some(app) = app_slot.borrow_mut().as_mut() {
            tick_offscreen(&mut app.world, &mut app.state, &mut app.renderer);
            post_camera_basis(&app.world, &last_basis);
            if let Some(&root) = app.state.viewer.resources.model.roots.first() {
                post_animation(&app.world, root, &last_anim);
            }
            let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
            if let Some(performance) = scope.performance() {
                let now = performance.now();
                let mut last = last_push.borrow_mut();
                if now - *last > 500.0 {
                    *last = now;
                    post(&WorkerMessage::Stats {
                        fps: app.world.resources.window.timing.frames_per_second,
                    });
                }
            }
        }
    });
}

fn mouse_button(button: u8) -> WinitMouseButton {
    match button {
        1 => WinitMouseButton::Middle,
        2 => WinitMouseButton::Right,
        _ => WinitMouseButton::Left,
    }
}

fn touch_phase(phase: protocol::TouchPhase) -> TouchPhase {
    match phase {
        protocol::TouchPhase::Started => TouchPhase::Started,
        protocol::TouchPhase::Moved => TouchPhase::Moved,
        protocol::TouchPhase::Ended => TouchPhase::Ended,
        protocol::TouchPhase::Cancelled => TouchPhase::Cancelled,
    }
}

fn bytes_from(data: &JsValue) -> Option<Vec<u8>> {
    bytes_with_key(data, BYTES_KEY)
}

fn bytes_with_key(data: &JsValue, key: &str) -> Option<Vec<u8>> {
    let value = js_sys::Reflect::get(data, &JsValue::from_str(key)).ok()?;
    let array = value.dyn_into::<js_sys::Uint8Array>().ok()?;
    Some(array.to_vec())
}

fn resources_from(data: &JsValue) -> HashMap<String, Vec<u8>> {
    let mut resources = HashMap::new();
    let Ok(value) = js_sys::Reflect::get(data, &JsValue::from_str(RESOURCES_KEY)) else {
        return resources;
    };
    if !value.is_object() {
        return resources;
    }
    let object: js_sys::Object = value.clone().unchecked_into();
    for key in js_sys::Object::keys(&object).iter() {
        if let Some(name) = key.as_string()
            && let Ok(entry) = js_sys::Reflect::get(&value, &key)
            && let Ok(array) = entry.dyn_into::<js_sys::Uint8Array>()
        {
            resources.insert(name, array.to_vec());
        }
    }
    resources
}

fn canvas_from(data: &JsValue) -> Option<OffscreenCanvas> {
    js_sys::Reflect::get(data, &JsValue::from_str(protocol::CANVAS_KEY))
        .ok()
        .and_then(|value| value.dyn_into::<OffscreenCanvas>().ok())
}

pub(crate) fn post(message: &WorkerMessage) {
    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    if let Ok(value) = serde_wasm_bindgen::to_value(message) {
        let _ = scope.post_message(&value);
    }
}

fn post_camera_basis(world: &World, last_basis: &Rc<RefCell<Option<[[f32; 3]; 3]>>>) {
    let Some(camera) = world.resources.active_camera else {
        return;
    };
    let Some(global) = world.core.get_global_transform(camera) else {
        return;
    };
    let right = global.right_vector();
    let up = global.up_vector();
    let forward = global.forward_vector();
    let basis = [
        [right.x, right.y, right.z],
        [up.x, up.y, up.z],
        [forward.x, forward.y, forward.z],
    ];
    let changed = last_basis
        .borrow()
        .map(|previous| basis_changed(&previous, &basis))
        .unwrap_or(true);
    if changed {
        *last_basis.borrow_mut() = Some(basis);
        post(&WorkerMessage::Camera {
            right: basis[0],
            up: basis[1],
            forward: basis[2],
        });
    }
}

fn basis_changed(a: &[[f32; 3]; 3], b: &[[f32; 3]; 3]) -> bool {
    a.iter()
        .zip(b)
        .any(|(x, y)| x.iter().zip(y).any(|(p, q)| (p - q).abs() > 0.0005))
}

fn animate(
    app: &mut App,
    action: impl Fn(&mut nightshade::ecs::animation::components::AnimationPlayer),
) {
    let roots = app.state.viewer.resources.model.roots.clone();
    for root in roots {
        if let Some(player) = app.world.core.get_animation_player_mut(root) {
            action(player);
        }
    }
}

fn post_animation(world: &World, root: Entity, last: &AnimState) {
    let Some(player) = world.core.get_animation_player(root) else {
        return;
    };
    let clip = player.current_clip.map(|index| index as u32);
    let duration = player.get_current_clip().map(|c| c.duration).unwrap_or(0.0);
    let state = (player.time, player.playing, clip);
    let changed = last
        .borrow()
        .map(|previous| {
            (previous.0 - state.0).abs() > 0.01 || previous.1 != state.1 || previous.2 != state.2
        })
        .unwrap_or(true);
    if changed {
        *last.borrow_mut() = Some(state);
        post(&WorkerMessage::Animation {
            time: player.time,
            duration,
            playing: player.playing,
            clip,
        });
    }
}

fn map_shading(mode: protocol::ShadingMode) -> nightshade::ecs::camera::components::ShadingMode {
    use nightshade::ecs::camera::components::ShadingMode as Engine;
    match mode {
        protocol::ShadingMode::Rendered => Engine::Rendered,
        protocol::ShadingMode::Solid => Engine::Solid,
        protocol::ShadingMode::Flat => Engine::Flat,
        protocol::ShadingMode::Wireframe => Engine::Wireframe,
    }
}

fn map_pbr(mode: protocol::PbrDebug) -> nightshade::ecs::graphics::resources::PbrDebugMode {
    use nightshade::ecs::graphics::resources::PbrDebugMode as Engine;
    match mode {
        protocol::PbrDebug::Off => Engine::None,
        protocol::PbrDebug::BaseColor => Engine::BaseColor,
        protocol::PbrDebug::Normal => Engine::Normal,
        protocol::PbrDebug::Metallic => Engine::Metallic,
        protocol::PbrDebug::Roughness => Engine::Roughness,
        protocol::PbrDebug::Occlusion => Engine::Occlusion,
        protocol::PbrDebug::Emissive => Engine::Emissive,
    }
}

fn map_tonemap(
    algorithm: protocol::Tonemap,
) -> nightshade::ecs::graphics::resources::TonemapAlgorithm {
    use nightshade::ecs::graphics::resources::TonemapAlgorithm as Engine;
    match algorithm {
        protocol::Tonemap::Aces => Engine::Aces,
        protocol::Tonemap::Reinhard => Engine::Reinhard,
        protocol::Tonemap::Uncharted2 => Engine::Uncharted2,
        protocol::Tonemap::AgX => Engine::AgX,
        protocol::Tonemap::Neutral => Engine::Neutral,
        protocol::Tonemap::None => Engine::None,
    }
}

fn context() -> String {
    let global = js_sys::global();
    js_sys::Reflect::get(&global, &JsValue::from_str("constructor"))
        .ok()
        .and_then(|constructor| js_sys::Reflect::get(&constructor, &JsValue::from_str("name")).ok())
        .and_then(|name| name.as_string())
        .unwrap_or_else(|| "worker".to_string())
}
