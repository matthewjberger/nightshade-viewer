mod behavior;
mod ecs;
mod graphics;
mod scene;
mod window;

pub use self::{behavior::*, scene::*};

pub use nalgebra_glm as math;

/// This is the entry point for the engine
pub fn start() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = winit::event_loop::EventLoop::builder().build()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut crate::Scene::default())?;
    Ok(())
}
