mod media_loader;
mod overlay;
mod renderer;

use anyhow::Result;
use env_logger::Env;
use media_loader::detect_media_type;
use overlay::OverlayApplication;
use std::env;
use std::path::Path;
use std::time::Duration;

fn main() -> Result<()> {
    // Initialize logger with default level INFO
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let args: Vec<String> = env::args().collect();

    // Parse command line arguments
    let (media_source, fps) = match args.len() {
        1 => {
            // No arguments provided
            eprintln!("Usage: {} <path> [fps]", args[0]);
            eprintln!("  path: Directory with images, GIF file, or APNG file");
            eprintln!("  fps: Frames per second (default: 30)");
            std::process::exit(1);
        }
        2 => {
            // One argument - the path
            let path = Path::new(&args[1]);
            let source = detect_media_type(path)?;
            log::info!("Using media source: {}", args[1]);
            (source, 30)
        }
        3 => {
            // Two arguments - path and FPS
            let path = Path::new(&args[1]);
            let source = detect_media_type(path)?;
            let fps = args[2].parse::<u64>().unwrap_or_else(|_| {
                log::warn!("Invalid FPS value '{}', using default 30", args[2]);
                30
            });
            log::info!("Using media source: {} with FPS: {}", args[1], fps);
            (source, fps)
        }
        _ => {
            eprintln!("Usage: {} <path> [fps]", args[0]);
            eprintln!("  path: Directory with images, GIF file, or APNG file");
            eprintln!("  fps: Frames per second (default: 30)");
            std::process::exit(1);
        }
    };

    let frame_interval = if fps > 0 {
        Duration::from_secs_f64(1.0 / fps as f64)
    } else {
        Duration::from_millis(33)
    };

    let mut app = OverlayApplication::new(media_source, frame_interval);
    app.run()?;

    Ok(())
}
