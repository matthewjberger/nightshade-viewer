mod ecs;
mod run;

pub mod modules {
    pub mod graphics;
    pub mod input;
    pub mod scene;
    pub mod ui;
    pub mod window;
}
pub use self::modules::*;

pub use run::start;
