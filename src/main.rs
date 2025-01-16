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
                default_value = "9003",
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
    let Options { command } = Options::from_args();
    match command {
        Some(Command::Run) | None => nightshade_core::run_frontend(),
        Some(Command::Server { port }) => {
            env_logger::init();
            nightshade_core::server::listen(port).await;
        }
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    nightshade_core::run_frontend();
}
