#![warn(clippy::all, rust_2018_idioms)]
// #![windows_subsystem = "windows"] // uncomment this to suppress terminal on windows

#[cfg(not(target_arch = "wasm32"))]
pub use cli::Options;

#[cfg(not(target_arch = "wasm32"))]
mod cli {
    use structopt::StructOpt;

    #[derive(Debug, StructOpt)]
    #[structopt(
        name = "Nightshade",
        about = "🥀 A fast and portable graphics engine 🦀🔍"
    )]
    pub struct Options {
        #[structopt(subcommand)]
        pub command: Option<Command>,
    }

    #[derive(Default, Debug, StructOpt)]
    pub enum Command {
        /// Launches the standalone desktop client.
        #[structopt(about = "Run the app")]
        #[default]
        Run,

        /// Starts the server to allow remote client connections.
        #[structopt(about = "Launch a server to accept connections from remote clients")]
        Server {
            /// The port for the server to listen on
            #[structopt(
                short,
                long,
                default_value = "9123",
                help = "The port the server will listen on"
            )]
            port: u16,
        },
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use cli::Command;
    use structopt::StructOpt;
    env_logger::init();
    let Options { command } = Options::from_args();
    match command {
        // Run the frontend
        Some(Command::Run) | None => {
            // Spawn IPC backend in a separate thread
            tokio::spawn(nightshade_engine::prelude::run_python_ipc_backend(9124));

            // Run frontend in main thread
            nightshade_engine::prelude::run_frontend()
        }

        // Run the RPC backend - fulfills requests from the frontend
        Some(Command::Server { port }) => {
            nightshade_engine::prelude::run_rpc_backend(port).await;
        }
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    nightshade_engine::prelude::run_frontend();
}
