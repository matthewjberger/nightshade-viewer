// #![windows_subsystem = "windows"] // uncomment this to suppress terminal on windows

fn main() -> Result<(), Box<dyn std::error::Error>> {
    nightshade_core::start()?;
    Ok(())
}
