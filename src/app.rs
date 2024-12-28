#[cfg(target_arch = "wasm32")]
use futures::channel::oneshot::Receiver;

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::{Duration, Instant};

#[cfg(target_arch = "wasm32")]
pub use web_time::{Duration, Instant};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[derive(Default)]
pub struct App {
    scene: crate::Scene,
    renderer: Option<crate::graphics::Renderer>,
    #[cfg(target_arch = "wasm32")]
    renderer_receiver: Option<Receiver<crate::graphics::Renderer>>,
    last_size: (u32, u32),
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let mut attributes = winit::window::Window::default_attributes();

        #[cfg(not(target_arch = "wasm32"))]
        {
            attributes = attributes.with_title("Hemlock");
        }

        #[allow(unused_assignments)]
        #[cfg(target_arch = "wasm32")]
        let (mut canvas_width, mut canvas_height) = (0, 0);

        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            let canvas = wgpu::web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .get_element_by_id("canvas")
                .unwrap()
                .dyn_into::<wgpu::web_sys::HtmlCanvasElement>()
                .unwrap();
            canvas_width = canvas.width();
            canvas_height = canvas.height();
            self.last_size = (canvas_width, canvas_height);
            attributes = attributes.with_canvas(Some(canvas));
        }

        if let Ok(window) = event_loop.create_window(attributes) {
            let first_window_handle = self.scene.resources.window.handle.is_none();
            let window_handle = std::sync::Arc::new(window);
            self.scene.resources.window.handle = Some(window_handle.clone());
            if first_window_handle {
                let gui_context = egui::Context::default();

                #[cfg(not(target_arch = "wasm32"))]
                {
                    let inner_size = window_handle.inner_size();
                    self.last_size = (inner_size.width, inner_size.height);
                }

                #[cfg(target_arch = "wasm32")]
                {
                    gui_context.set_pixels_per_point(window_handle.scale_factor() as f32);
                }

                let viewport_id = gui_context.viewport_id();
                let gui_state = egui_winit::State::new(
                    gui_context,
                    viewport_id,
                    &window_handle,
                    Some(window_handle.scale_factor() as _),
                    Some(winit::window::Theme::Dark),
                    None,
                );

                #[cfg(not(target_arch = "wasm32"))]
                let (width, height) = (
                    window_handle.inner_size().width,
                    window_handle.inner_size().height,
                );

                #[cfg(not(target_arch = "wasm32"))]
                {
                    env_logger::init();
                    let renderer = pollster::block_on(async move {
                        crate::graphics::Renderer::new(window_handle.clone(), width, height).await
                    });
                    self.renderer = Some(renderer);
                }

                #[cfg(target_arch = "wasm32")]
                {
                    let (sender, receiver) = futures::channel::oneshot::channel();
                    self.renderer_receiver = Some(receiver);
                    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
                    console_log::init().expect("Failed to initialize logger!");
                    log::info!("Canvas dimensions: ({canvas_width} x {canvas_height})");
                    wasm_bindgen_futures::spawn_local(async move {
                        let renderer = crate::graphics::Renderer::new(
                            window_handle.clone(),
                            canvas_width,
                            canvas_height,
                        )
                        .await;
                        if sender.send(renderer).is_err() {
                            log::error!("Failed to create and send renderer!");
                        }
                    });
                }

                self.scene.resources.user_interface.gui_state = Some(gui_state);
                self.scene.resources.frame_timing.last_frame_start_instant = Some(Instant::now());
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        #[cfg(target_arch = "wasm32")]
        {
            let mut renderer_received = false;
            if let Some(receiver) = self.renderer_receiver.as_mut() {
                if let Ok(Some(renderer)) = receiver.try_recv() {
                    self.renderer = Some(renderer);
                    renderer_received = true;
                }
            }
            if renderer_received {
                self.renderer_receiver = None;
            }
        }

        let Self {
            renderer: Some(renderer),
            ..
        } = self
        else {
            return;
        };

        // Receive gui window event
        if let Some(gui_state) = &mut self.scene.resources.user_interface.gui_state {
            if let Some(window_handle) = self.scene.resources.window.handle.as_ref() {
                if gui_state.on_window_event(window_handle, &event).consumed {
                    return;
                }
            }
        }

        // If the gui didn't consume the event, handle it
        match event {
            winit::event::WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key: winit::keyboard::PhysicalKey::Code(key_code),
                        ..
                    },
                ..
            } => {
                // Exit by pressing the escape key
                if matches!(key_code, winit::keyboard::KeyCode::Escape) {
                    event_loop.exit();
                }
            }
            winit::event::WindowEvent::Resized(winit::dpi::PhysicalSize { width, height }) => {
                log::info!("Resizing renderer surface to: ({width}, {height})");
                renderer.resize(width, height);
                self.last_size = (width, height);
            }
            winit::event::WindowEvent::CloseRequested => {
                log::info!("Close requested. Exiting...");
                event_loop.exit();
            }
            winit::event::WindowEvent::RedrawRequested => {
                crate::run_systems(&mut self.scene);

                let (ui, textures_delta, shapes, pixels_per_point) =
                    match crate::ui_system(&mut self.scene) {
                        Some(value) => value,
                        None => return,
                    };

                let paint_jobs = ui.tessellate(shapes, pixels_per_point);

                if let Some(window_handle) = self.scene.resources.window.handle.as_ref() {
                    let screen_descriptor = {
                        let (width, height) = self.last_size;
                        egui_wgpu::ScreenDescriptor {
                            size_in_pixels: [width, height],
                            pixels_per_point: window_handle.scale_factor() as f32,
                        }
                    };

                    renderer.render_frame(
                        screen_descriptor,
                        paint_jobs,
                        textures_delta,
                        self.scene.resources.frame_timing.delta_time,
                    );
                }
            }
            _ => (),
        }

        if let Some(window_handle) = self.scene.resources.window.handle.as_mut() {
            window_handle.request_redraw();
        }
    }
}
