mod context;
mod ecs;
mod graphics;
mod input;
mod paint;
mod run;
mod ui;
mod window;

pub use context::Context;
pub use run::start;

pub mod prelude {
    pub use crate::context::*;
    pub use crate::graphics::*;
    pub use crate::input::*;
    pub use crate::paint::*;
    pub use crate::ui::*;
    pub use crate::window::*;
}
