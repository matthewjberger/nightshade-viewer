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

        let Ok(window) = event_loop.create_window(attributes) else {
            return;
        };

        let window_handle = std::sync::Arc::new(window);
        self.resources.window.handle = Some(window_handle.clone());

        crate::commands::initialize(self);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        #[cfg(target_arch = "wasm32")]
        crate::systems::receive_renderer_system(self);

        if self.resources.graphics.renderer.is_none() {
            return;
        }

        if self.resources.window.should_exit
            || matches!(event, winit::event::WindowEvent::CloseRequested)
        {
            event_loop.exit();
            return;
        }

        if let Some(gui_state) = &mut self.resources.user_interface.state {
            if let Some(window_handle) = self.resources.window.handle.as_ref() {
                if gui_state.on_window_event(window_handle, &event).consumed {
                    return;
                }
            }
        }

        crate::step(self, &event);

        if let Some(window_handle) = self.resources.window.handle.as_mut() {
            window_handle.request_redraw();
        }
    }
}
