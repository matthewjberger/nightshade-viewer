use crate::ecs::ViewerWorld;
use crate::systems;
use nightshade::prelude::*;

/// The application root. Holds the user-side ECS world (`ViewerWorld`) and
/// forwards each `State` hook to system functions in `src/systems/`.
#[derive(Default)]
pub struct Viewer {
    pub viewer: ViewerWorld,
}

impl State for Viewer {
    fn initialize(&mut self, world: &mut World) {
        systems::setup::spawn(&mut self.viewer, world);
    }

    fn run_systems(&mut self, world: &mut World) {
        systems::camera::update(&mut self.viewer, world);
        systems::browsers::ensure_indices(&self.viewer);
        systems::browsers::poll(&mut self.viewer);
        systems::load::poll(&mut self.viewer, world);
        systems::load::poll_agent_loads(&mut self.viewer, world);
        systems::load::flush_report(&mut self.viewer, world);
        systems::picking::apply(&mut self.viewer, world);
        systems::scene::sync(&mut self.viewer, world);
    }
}
