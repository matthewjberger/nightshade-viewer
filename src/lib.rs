mod ecs;

pub mod modules {
    pub mod graphics;
    pub mod input;
    pub mod run;
    pub mod scene;
    pub mod ui;
    pub mod window;
}

pub use modules::run::start;
