use crate::input;

/// This is the entry point for the engine
pub fn start() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = winit::event_loop::EventLoop::builder().build()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut crate::scene::Context::default())?;
    Ok(())
}

/// This is the main loop, driven by winit window events.
/// Resources are updated and then systems are triggered continuously.
pub(crate) fn step(context: &mut crate::scene::Context, event: &winit::event::WindowEvent) {
    // On wasm, the renderer is received from an async task
    // and will not be available in the cycles prior to that
    #[cfg(target_arch = "wasm32")]
    crate::graphics::systems::receive_renderer(context);

    // The renderer should be available before running any systems
    if context.resources.graphics.renderer.is_none() {
        return;
    }

    match event {
        // Update every frame
        winit::event::WindowEvent::RedrawRequested => {
            crate::scene::systems::ensure_main_camera(context);
            crate::window::systems::update_frame_timing(context);
            crate::ui::systems::ensure_tile_tree(context);
            input::escape_key_exit_system(context);
            crate::scene::systems::orbital_camera(context);
            crate::scene::systems::wasd_keyboard_controls_system(context);
            crate::scene::systems::calculate_global_transforms_system(context);
            crate::ui::systems::render_ui(context);
            crate::graphics::systems::render_frame(context);
            input::reset_input_system(context);
        }
        // Receive events, which populate the world resources
        event => {
            crate::ui::events::receive_ui_event(context, event);
            crate::window::events::receive_resize_event(context, event);
            input::receive_keyboard_event(context, event);
            input::receive_mouse_event(context, event);
        }
    }
}
