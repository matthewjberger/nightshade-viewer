mod ecs;
mod graphics;
mod input;
mod paint;
mod run;
mod scene;
mod ui;
mod window;

pub use run::start;
pub use scene::Context;

pub mod prelude {
    pub use crate::graphics::*;
    pub use crate::input::*;
    pub use crate::paint::*;
    pub use crate::scene::*;
    pub use crate::ui::*;
    pub use crate::window::*;
}
