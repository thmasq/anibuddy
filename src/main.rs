mod image_loader;
mod overlay;
mod renderer;

use anyhow::Result;
use env_logger::Env;
use include_dir::{Dir, include_dir};
use overlay::OverlayApplication;
use std::env;
use std::time::Duration;

static FRAMES_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/frames");

fn main() -> Result<()> {
    // Initialize logger with default level INFO
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let args: Vec<String> = env::args().collect();
    let fps = if args.len() > 1 {
        match args[1].parse::<u64>() {
            Ok(val) => {
                log::info!("Setting FPS to {}", val);
                val
            }
            Err(_) => {
                log::warn!("Invalid FPS value provided, using default");
                30
            }
        }
    } else {
        log::info!("Using default FPS: 30");
        30
    };

    let frame_interval = if fps > 0 {
        Duration::from_secs_f64(1.0 / fps as f64)
    } else {
        Duration::from_millis(33)
    };

    let mut app = OverlayApplication::new_embedded(&FRAMES_DIR, frame_interval);
    app.run()?;

    Ok(())
}
