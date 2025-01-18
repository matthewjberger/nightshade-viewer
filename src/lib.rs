pub mod api;
pub mod context;
pub mod ecs;
pub mod graphics;
pub mod input;
pub mod paint;
pub mod rpc;
pub mod run;
pub mod ui;
pub mod window;

#[cfg(not(target_arch = "wasm32"))]
pub mod ipc;

pub mod prelude {
    pub use crate::api::*;
    pub use crate::context::*;
    pub use crate::rpc::*;
    pub use crate::run::run_frontend;
    pub use crate::ui::*;

    #[cfg(not(target_arch = "wasm32"))]
    pub use crate::ipc::{run_python_ipc_backend, IpcCommand, IpcError, IpcEvent, IpcMessage};
}

pub use log;
pub use nalgebra_glm;
pub use serde_json;

#[cfg(not(target_arch = "wasm32"))]
pub use tokio_tungstenite;
