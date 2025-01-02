#[derive(Default)]
pub struct Window {
    /// The raw window handle
    pub handle: Option<std::sync::Arc<winit::window::Window>>,

    /// Should the program exit next frame
    pub should_exit: bool,

    /// The number of frames rendered per second
    pub frames_per_second: f32,

    /// The time between the last frame and the current frame
    pub delta_time: f32,

    /// The time the current frame was started
    pub last_frame_start_instant: Option<web_time::Instant>,

    /// When the current frame started
    pub current_frame_start_instant: Option<web_time::Instant>,

    /// When the initial frame started, when the application starts up
    pub initial_frame_start_instant: Option<web_time::Instant>,

    /// A monotonically increasing counter incremented each frame
    pub frame_counter: u32,

    /// Milliseconds that the process has been running continuously
    pub uptime_milliseconds: u64,
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

impl winit::application::ApplicationHandler for crate::scene::Context {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut attributes = winit::window::Window::default_attributes();

        attributes.title = "Nightshade".to_string();

        match image::load_from_memory(include_bytes!("icon/nightshade.png")) {
            Ok(image) => {
                let image = image.to_rgba8();
                let (width, height) = image.dimensions();
                if let Ok(icon) = winit::window::Icon::from_rgba(image.to_vec(), width, height) {
                    attributes.window_icon = Some(icon);
                }
            }
            Err(e) => {
                eprintln!("Failed to load icon: {e}");
            }
        }

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

        let Ok(window) = event_loop.create_window(dbg!(attributes)) else {
            return;
        };

        let window_handle = std::sync::Arc::new(window);
        self.resources.window.handle = Some(window_handle.clone());

        initialize(self);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        if self.resources.window.should_exit
            || matches!(event, winit::event::WindowEvent::CloseRequested)
        {
            event_loop.exit();
            return;
        }

        crate::run::step(self, &event);

        // Ensure we cycle frames continuously by requesting a redraw at the end of each frame
        if let Some(window_handle) = self.resources.window.handle.as_mut() {
            window_handle.request_redraw();
        }
    }
}

/// Initializes context resources on startup
pub fn initialize(context: &mut crate::scene::Context) {
    let window_handle = {
        let Some(window_handle) = context.resources.window.handle.as_mut() else {
            return;
        };
        window_handle.clone()
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        let inner_size = window_handle.inner_size();
        context.resources.graphics.viewport_size = (inner_size.width, inner_size.height);
    }

    let gui_context = egui::Context::default();

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
            crate::graphics::create_renderer_async(window_handle.clone(), width, height).await
        });
        context.resources.graphics.renderer = Some(renderer);
    }

    #[cfg(target_arch = "wasm32")]
    {
        let (sender, receiver) = futures::channel::oneshot::channel();
        context.resources.graphics.renderer_receiver = Some(receiver);
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init().expect("Failed to initialize logger!");
        let (canvas_width, canvas_height) = context.resources.graphics.viewport_size;
        log::info!("Canvas dimensions: ({canvas_width} x {canvas_height})");
        wasm_bindgen_futures::spawn_local(async move {
            let renderer = crate::graphics::create_renderer_async(
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

    context.resources.user_interface.state = Some(gui_state);
    context.resources.window.last_frame_start_instant = Some(web_time::Instant::now());
}

pub fn receive_resize_event(
    context: &mut crate::scene::Context,
    event: &winit::event::WindowEvent,
) {
    let winit::event::WindowEvent::Resized(winit::dpi::PhysicalSize { width, height }) = event
    else {
        return;
    };
    crate::window::resize_viewport(context, *width, *height);
}

/// Handles viewport resizing, such as when the window is resized by the user
pub fn resize_viewport(context: &mut crate::scene::Context, width: u32, height: u32) {
    crate::graphics::resize_renderer(context, width, height);
    crate::ui::resize_ui(context);
}

/// Queries for the display viewport's aspect ratio
pub fn query_viewport_aspect_ratio(context: &crate::scene::Context) -> Option<f32> {
    let Some(renderer) = &context.resources.graphics.renderer else {
        return None;
    };
    let surface_config = &renderer.gpu.surface_config;
    let aspect_ratio = surface_config.width as f32 / surface_config.height.max(1) as f32;
    Some(aspect_ratio)
}

/// Calculates and refreshes frame timing values such as delta time
pub fn update_frame_timing_system(context: &mut crate::scene::Context) {
    let now = web_time::Instant::now();

    let crate::scene::Context {
        resources:
            crate::scene::Resources {
                window:
                    crate::window::Window {
                        delta_time,
                        last_frame_start_instant,
                        current_frame_start_instant,
                        initial_frame_start_instant,
                        frame_counter,
                        uptime_milliseconds,
                        frames_per_second,
                        ..
                    },
                ..
            },
        ..
    } = context;

    // Capture first instant
    if initial_frame_start_instant.is_none() {
        *initial_frame_start_instant = Some(now);
    }

    // Delta time
    *delta_time =
        last_frame_start_instant.map_or(0.0, |last_frame| (now - last_frame).as_secs_f32());

    // Last frame start
    *last_frame_start_instant = Some(now);

    // Current frame start
    if current_frame_start_instant.is_none() {
        *current_frame_start_instant = Some(now);
    }

    // Calculate uptime
    if let Some(app_start) = *initial_frame_start_instant {
        *uptime_milliseconds = (now - app_start).as_millis() as u64;
    }

    // Calculate frames per second
    *frame_counter += 1;
    match current_frame_start_instant.as_ref() {
        Some(start) => {
            if (now - *start).as_secs_f32() >= 1.0 {
                *frames_per_second = *frame_counter as f32;
                *frame_counter = 0;
                *current_frame_start_instant = Some(now);
            }
        }
        None => {
            *current_frame_start_instant = Some(now);
        }
    }
}
