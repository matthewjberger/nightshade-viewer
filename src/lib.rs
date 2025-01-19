mod api;
mod context;
mod ecs;
mod graphics;
mod input;
mod paint;
mod rpc;
mod run;
mod ui;
mod window;

// The backend daemon cannot be run in a browser
#[cfg(not(target_arch = "wasm32"))]
pub mod server;

pub use context::Context;
pub use run::run_frontend;

pub mod prelude {
    pub use crate::api::{push_command, push_event, Command, Event};
    pub use crate::context::*;
    pub use crate::input::*;
    pub use crate::paint::*;
    pub use crate::rpc::{RpcCommand, RpcError, RpcEvent, RpcMessage};
    pub use crate::ui::*;
    pub use crate::window::*;
}
