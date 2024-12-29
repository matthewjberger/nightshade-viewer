#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

impl winit::application::ApplicationHandler for crate::Scene {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut attributes = winit::window::Window::default_attributes();

        // On wasm, the window attributes have to include the canvas element
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            let Some(window) = wgpu::web_sys::window() else {
                return;
            };
            let Some(document) = window.document() else {
                return;
            };
            let Some(element) = document.get_element_by_id("canvas") else {
                return;
            };
            let Ok(canvas) = element.dyn_into::<wgpu::web_sys::HtmlCanvasElement>() else {
                return;
            };
            self.resources.graphics.viewport_size = (canvas.width(), canvas.height());
            attributes = attributes.with_canvas(Some(canvas));
        }

        if let Ok(window) = event_loop.create_window(attributes) {
            let window_handle = std::sync::Arc::new(window);
            self.resources.window.handle = Some(window_handle.clone());
            run_initialization_systems(self);
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
            if let Some(receiver) = self.resources.graphics.renderer_receiver.as_mut() {
                if let Ok(Some(renderer)) = receiver.try_recv() {
                    self.resources.graphics.renderer = Some(renderer);
                    renderer_received = true;
                }
            }
            if renderer_received {
                self.resources.graphics.renderer_receiver = None;
            }
        }

        if self.resources.graphics.renderer.is_none() {
            return;
        }

        // Receive gui window event
        if let Some(gui_state) = &mut self.resources.user_interface.state {
            if let Some(window_handle) = self.resources.window.handle.as_ref() {
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
                if let Some(renderer) = self.resources.graphics.renderer.as_mut() {
                    renderer.resize(width, height);
                }
                self.resources.graphics.viewport_size = (width, height);
            }
            winit::event::WindowEvent::CloseRequested => {
                log::info!("Close requested. Exiting...");
                event_loop.exit();
            }
            winit::event::WindowEvent::RedrawRequested => {
                crate::run_systems(self);
            }
            _ => (),
        }

        if let Some(window_handle) = self.resources.window.handle.as_mut() {
            window_handle.request_redraw();
        }
    }
}

fn run_initialization_systems(scene: &mut crate::Scene) {
    let window_handle = {
        let Some(window_handle) = scene.resources.window.handle.as_mut() else {
            return;
        };
        window_handle.clone()
    };

    let gui_context = egui::Context::default();

    #[cfg(not(target_arch = "wasm32"))]
    {
        let inner_size = window_handle.inner_size();
        scene.resources.graphics.viewport_size = (inner_size.width, inner_size.height);
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
        scene.resources.graphics.renderer = Some(renderer);
    }

    #[cfg(target_arch = "wasm32")]
    {
        let (sender, receiver) = futures::channel::oneshot::channel();
        scene.resources.graphics.renderer_receiver = Some(receiver);
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init().expect("Failed to initialize logger!");
        let (canvas_width, canvas_height) = scene.resources.graphics.viewport_size;
        log::info!("Canvas dimensions: ({canvas_width} x {canvas_height})");
        wasm_bindgen_futures::spawn_local(async move {
            let renderer =
                crate::graphics::Renderer::new(window_handle.clone(), canvas_width, canvas_height)
                    .await;
            if sender.send(renderer).is_err() {
                log::error!("Failed to create and send renderer!");
            }
        });
    }

    scene.resources.user_interface.state = Some(gui_state);
    scene.resources.frame_timing.last_frame_start_instant = Some(web_time::Instant::now());
}
