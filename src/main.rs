// #![windows_subsystem = "windows"] // uncomment this to suppress terminal on windows

fn main() -> Result<(), winit::error::EventLoopError> {
    nightshade_core::start()?;
    Ok(())
}
