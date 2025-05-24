mod config;
mod delta_compression;
mod media_loader;
mod overlay;
mod renderer;

use anyhow::{Result, anyhow};
use clap::Parser;
use config::{Config, PresetConfig, is_likely_path};
use env_logger::Env;
use media_loader::{MediaSource, detect_media_type};
use overlay::OverlayApplication;
use std::path::Path;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "konata-dance-rs")]
#[command(about = "A dancing overlay application with delta compression support")]
#[command(
    long_about = r#"A dancing overlay application that can display animated sequences from:
- Directories containing image files (PNG, JPG, JPEG)
- GIF files
- APNG files
- Named presets from config file

Supports delta compression to reduce memory usage by 50-90% for animations 
with small changes between frames."#
)]
struct Args {
    /// Path to directory with images, GIF file, APNG file, or preset name
    path_or_preset: Option<String>,

    /// Frames per second (overrides preset FPS if specified)
    #[arg(short, long)]
    fps: Option<u64>,

    /// Enable delta compression for memory efficiency
    #[arg(short, long)]
    compress: bool,

    /// List available presets and exit
    #[arg(long)]
    list_presets: bool,
}

fn main() -> Result<()> {
    // Initialize logger with default level None
    env_logger::Builder::from_env(Env::default().default_filter_or("none")).init();

    let args = Args::parse();

    // Load config file
    let config = Config::load()?;

    // Handle list presets command
    if args.list_presets {
        print_presets(&config);
        return Ok(());
    }

    // Determine media source and fps
    let (media_source, fps) = match args.path_or_preset {
        Some(path_or_preset) => resolve_path_or_preset(&config, &path_or_preset, args.fps)?,
        None => {
            // No path/preset specified, try to use default preset
            match get_default_preset(&config, args.fps) {
                Ok((source, fps)) => (source, fps),
                Err(_) => {
                    eprintln!(
                        "Error: No path or preset specified and no default preset configured."
                    );
                    eprintln!();
                    print_usage_hint(&config);
                    std::process::exit(1);
                }
            }
        }
    };

    let frame_interval = create_frame_interval(fps);

    if args.compress {
        log::info!("Starting application with delta compression enabled");
    } else {
        log::info!("Starting application with standard (uncompressed) mode");
    }

    let mut app = OverlayApplication::new(media_source, frame_interval, args.compress);
    app.run()?;

    Ok(())
}

/// Resolve a path or preset name to a MediaSource and FPS
fn resolve_path_or_preset(
    config: &Option<Config>,
    path_or_preset: &str,
    fps_override: Option<u64>,
) -> Result<(MediaSource, u64)> {
    if is_likely_path(path_or_preset) {
        // Treat as path
        let media_source = create_media_source_from_path(path_or_preset)?;
        let fps = fps_override.unwrap_or(30);
        log::info!("Using path: {} (fps: {})", path_or_preset, fps);
        Ok((media_source, fps))
    } else if let Some(cfg) = config {
        // Try as preset first
        if let Some(preset) = cfg.get_preset(path_or_preset) {
            let media_source = create_media_source_from_preset(preset)?;
            let fps = fps_override.unwrap_or(preset.fps.unwrap_or(30));

            if fps_override.is_some() {
                log::info!(
                    "Using preset '{}': {} (fps: {} - overridden)",
                    path_or_preset,
                    preset.path,
                    fps
                );
            } else {
                log::info!(
                    "Using preset '{}': {} (fps: {})",
                    path_or_preset,
                    preset.path,
                    fps
                );
            }

            Ok((media_source, fps))
        } else {
            // Not a preset, try as path
            try_path_fallback(cfg, path_or_preset, fps_override)
        }
    } else {
        // No config file, treat as path
        let media_source = create_media_source_from_path(path_or_preset)?;
        let fps = fps_override.unwrap_or(30);
        log::info!("Using path: {} (fps: {})", path_or_preset, fps);
        Ok((media_source, fps))
    }
}

/// Get the default preset if available
fn get_default_preset(
    config: &Option<Config>,
    fps_override: Option<u64>,
) -> Result<(MediaSource, u64)> {
    if let Some(cfg) = config {
        if let Some(default_preset) = cfg.get_default() {
            let media_source = create_media_source_from_preset(default_preset)?;
            let fps = fps_override.unwrap_or(default_preset.fps.unwrap_or(30));

            if fps_override.is_some() {
                log::info!(
                    "Using default preset: {} (fps: {} - overridden)",
                    default_preset.path,
                    fps
                );
            } else {
                log::info!(
                    "Using default preset: {} (fps: {})",
                    default_preset.path,
                    fps
                );
            }

            Ok((media_source, fps))
        } else {
            Err(anyhow!("No default preset configured"))
        }
    } else {
        Err(anyhow!("No config file found"))
    }
}

/// Try to use the argument as a path when it's not found as a preset
fn try_path_fallback(
    config: &Config,
    arg: &str,
    fps_override: Option<u64>,
) -> Result<(MediaSource, u64)> {
    let path = Path::new(arg);
    if !path.exists() {
        let available_presets = config.list_presets();
        let mut error_msg = format!(
            "No preset named '{}' found and path '{}' does not exist",
            arg, arg
        );

        if !available_presets.is_empty() {
            error_msg.push_str(&format!(
                "\nAvailable presets: {}",
                available_presets.join(", ")
            ));
        }

        return Err(anyhow!(error_msg));
    }

    let media_source = detect_media_type(path)?;
    let fps = fps_override.unwrap_or(30);
    log::info!("Using path: {} (fps: {})", arg, fps);
    Ok((media_source, fps))
}

/// Create a MediaSource from a preset configuration
fn create_media_source_from_preset(preset: &PresetConfig) -> Result<MediaSource> {
    let path = Path::new(&preset.path);
    if !path.exists() {
        return Err(anyhow!(
            "Preset points to non-existent path '{}'",
            preset.path
        ));
    }
    detect_media_type(path)
}

/// Create a MediaSource from a path string
fn create_media_source_from_path(path_str: &str) -> Result<MediaSource> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(anyhow!("Path '{}' does not exist", path_str));
    }
    detect_media_type(path)
}

/// Create a Duration for the frame interval based on FPS
fn create_frame_interval(fps: u64) -> Duration {
    if fps > 0 {
        Duration::from_secs_f64(1.0 / fps as f64)
    } else {
        Duration::from_millis(33)
    }
}

/// Print available presets
fn print_presets(config: &Option<Config>) {
    if let Some(cfg) = config {
        let presets = cfg.list_presets();
        if !presets.is_empty() {
            println!("Available presets:");
            for preset_name in presets {
                if let Some(preset) = cfg.get_preset(&preset_name) {
                    println!(
                        "  {} -> {} (fps: {})",
                        preset_name,
                        preset.path,
                        preset.fps.unwrap_or(30)
                    );
                }
            }
        } else {
            println!("No presets configured.");
        }
    } else {
        println!("No config file found.");
        println!("Create ~/.config/konata-dance/config.toml to configure presets.");
    }
}

/// Print usage hints and examples
fn print_usage_hint(config: &Option<Config>) {
    println!("Examples:");
    println!("  konata-dance-rs ./frames              # Use frames directory");
    println!("  konata-dance-rs --compress ./frames   # Use frames directory with compression");
    println!("  konata-dance-rs animation.gif         # Use GIF file");
    println!("  konata-dance-rs -c animation.gif      # Use GIF file with compression");
    println!("  konata-dance-rs konata                # Use 'konata' preset");
    println!("  konata-dance-rs --compress konata     # Use 'konata' preset with compression");
    println!("  konata-dance-rs ./frames --fps 60     # Use frames directory at 60 FPS");
    println!(
        "  konata-dance-rs -c ./frames --fps 60  # Use frames directory at 60 FPS with compression"
    );
    println!();
    println!("Controls:");
    println!();

    if let Some(config) = config {
        let presets = config.list_presets();
        if !presets.is_empty() {
            println!("Available presets: {}", presets.join(", "));
            println!("Use --list-presets to see preset details.");
        } else {
            println!("No presets configured.");
        }
    } else {
        println!("No config file found. Create ~/.config/konata-dance/config.toml to use presets.");
    }

    println!();
    println!("Config file format (~/.config/konata-dance/config.toml):");
    println!("[default]");
    println!("path = \"/path/to/default/animation\"");
    println!("fps = 30");
    println!();
    println!("[konata]");
    println!("path = \"/path/to/konata/frames\"");
    println!("fps = 24");
    println!();
    println!("[1]");
    println!("path = \"/path/to/animation1.gif\"");
    println!("fps = 60");
    println!();
    println!("Delta Compression:");
    println!("Delta compression reduces memory usage by storing only the differences");
    println!("between consecutive frames. This is especially effective for animations");
    println!("with small changes between frames, potentially reducing memory usage");
    println!("by 50-90% depending on the content.");
}
