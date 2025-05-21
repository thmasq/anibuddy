mod image_loader;
mod overlay;
mod renderer;

use anyhow::Result;
use env_logger::Env;
use overlay::OverlayApplication;
use std::path::PathBuf;

fn main() -> Result<()> {
    // Initialize logger with default level INFO
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // Directory containing PNG sequence (hardcoded for now)
    let image_dir = PathBuf::from("./frames");

    let mut app = OverlayApplication::new(image_dir);
    app.run()?;

    Ok(())
}
