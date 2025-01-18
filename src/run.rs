use crate::{api, context, graphics, input, rpc, ui, window};

/// This is the entry point for the engine
pub fn run_frontend() {
    let Ok(event_loop) = winit::event_loop::EventLoop::builder().build() else {
        eprintln!("Failed to create event loop!");
        return;
    };
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    if let Err(error) = event_loop.run_app(&mut context::Context::default()) {
        eprintln!("Failed to run app: {error}");
    }
}

/// This is the main loop, driven by winit window events.
/// Resources are updated and then systems are triggered continuously.
pub(crate) fn step(context: &mut context::Context, event: &winit::event::WindowEvent) {
    match event {
        winit::event::WindowEvent::RedrawRequested => {
            // On wasm, the renderer is received from an async task
            // and will not be available in the cycles prior to that
            #[cfg(target_arch = "wasm32")]
            graphics::receive_renderer_system(context);

            // The renderer should be available before running any systems
            if context.resources.graphics.renderer.is_none() {
                return;
            }

            // start of frame, must come first
            window::update_frame_timing_system(context);

            // process IPC commands first
            #[cfg(not(target_arch = "wasm32"))]
            crate::ipc::process_ipc_commands_system(context);

            // external network events
            rpc::receive_rpc_events_system(context);

            // route queued engine events to their domains
            api::route_events_system(context);

            // execute queued engine commands
            api::execute_commands_system(context);

            // Ensure at least one camera is present
            context::ensure_main_camera_system(context);

            // Ensure cameras have a local transform component
            context::ensure_camera_transform_system(context);

            // Ensure that the tile tree is never fully empty
            ui::ensure_tile_tree_system(context);

            // Press escape to exit
            input::escape_key_exit_system(context);

            // Active camera rotation mouse dragging controls
            context::look_camera_system(context);

            // WASD keyboard controls for the active camera
            context::wasd_keyboard_controls_system(context);

            // Uses entity parent hierarchy to update global transforms
            context::update_global_transforms_system(context);

            // Create the UI in memory
            ui::create_ui_system(context);

            // Render the frame to the screen
            graphics::render_frame_system(context);

            // Reset input states at the end of the frame
            input::reset_input_system(context);
        }
        event => {
            // TODO: move this out to a list in the context resources
            //       It can be dequeued by another system
            ui::receive_ui_event(context, event);
            window::receive_window_event(context, event);
            input::receive_input_event(context, event);
        }
    }
}

/// Entry point for the engine with a pre-configured context
pub fn run_frontend_with_context(mut context: context::Context) {
    let Ok(event_loop) = winit::event_loop::EventLoop::builder().build() else {
        eprintln!("Failed to create event loop!");
        return;
    };
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    if let Err(error) = event_loop.run_app(&mut context) {
        eprintln!("Failed to run app: {error}");
    }
}
