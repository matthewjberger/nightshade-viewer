use crate::{graphics, input, scene, ui, window};

/// This is the entry point for the engine
pub fn start() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = winit::event_loop::EventLoop::builder().build()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut scene::Context::default())?;
    Ok(())
}

/// This is the main loop, driven by winit window events.
/// Resources are updated and then systems are triggered continuously.
pub(crate) fn step(context: &mut scene::Context, event: &winit::event::WindowEvent) {
    // On wasm, the renderer is received from an async task
    // and will not be available in the cycles prior to that
    #[cfg(target_arch = "wasm32")]
    graphics::receive_renderer_system(context);

    // The renderer should be available before running any systems
    if context.resources.graphics.renderer.is_none() {
        return;
    }

    match event {
        // Update every frame
        winit::event::WindowEvent::RedrawRequested => {
            scene::ensure_main_camera_system(context);
            window::update_frame_timing_system(context);
            ui::ensure_tile_tree_system(context);
            input::escape_key_exit_system(context);
            scene::look_camera_system(context);
            scene::wasd_keyboard_controls_system(context);
            scene::update_global_transforms_system(context);
            ui::render_ui_system(context);
            graphics::render_frame_system(context);
            input::reset_input_system(context);
        }
        // Receive events, which populate the world resources
        event => {
            ui::receive_ui_event(context, event);
            window::receive_resize_event(context, event);
            input::receive_keyboard_event(context, event);
            input::receive_mouse_event(context, event);
        }
    }
}
