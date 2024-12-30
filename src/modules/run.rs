/// This is the entry point for the engine
pub fn start() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = winit::event_loop::EventLoop::builder().build()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut crate::modules::scene::Context::default())?;
    Ok(())
}

/// This is the main loop, driven by winit window events.
/// Resources are updated and then systems are triggered continuously.
pub fn step(context: &mut crate::modules::scene::Context, event: &winit::event::WindowEvent) {
    // On wasm, the renderer is received from an async task
    // and will not be available in the cycles prior to that
    #[cfg(target_arch = "wasm32")]
    crate::modules::graphics::systems::receive_renderer(context);

    // The renderer should be available before running any systems
    if context.resources.graphics.renderer.is_none() {
        return;
    }

    match event {
        // Update every frame
        winit::event::WindowEvent::RedrawRequested => {
            crate::modules::window::systems::update_frame_timing(context);
            crate::modules::ui::systems::ensure_tile_tree(context);

            crate::modules::input::systems::escape_key_exit(context);

            crate::modules::ui::systems::render_ui(context);
            crate::modules::graphics::systems::render_frame(context);

            crate::modules::input::systems::reset_input(context);
        }
        // Receive events, which populate the world resources
        event => {
            crate::modules::ui::events::receive_ui_event(context, event);
            crate::modules::window::events::receive_resize_event(context, event);
            crate::modules::input::events::receive_keyboard_event(context, event);
            crate::modules::input::events::receive_mouse_event(context, event);
        }
    }
}
