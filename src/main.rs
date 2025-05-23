mod config;
mod media_loader;
mod overlay;
mod renderer;

use anyhow::{Result, anyhow};
use config::{Config, PresetConfig, is_likely_path};
use env_logger::Env;
use media_loader::{MediaSource, detect_media_type};
use overlay::OverlayApplication;
use std::env;
use std::path::Path;
use std::time::Duration;

fn main() -> Result<()> {
    // Initialize logger with default level INFO
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let args: Vec<String> = env::args().collect();

    // Load config file
    let config = Config::load()?;

    // Parse command line arguments and determine media source and fps
    let (media_source, fps) = match args.len() {
        1 => {
            // No arguments provided - try to use default preset
            match handle_no_arguments(&config) {
                Ok(result) => result,
                Err(_) => {
                    print_usage(&args[0], &config);
                    std::process::exit(1);
                }
            }
        }
        2 => {
            // One argument - could be preset name or path
            match handle_single_argument(&config, &args[1]) {
                Ok(result) => result,
                Err(err) => {
                    eprintln!("Error: {}", err);
                    std::process::exit(1);
                }
            }
        }
        3 => {
            // Two arguments - path/preset and FPS
            let fps = parse_fps(&args[2]);
            match handle_argument_with_fps(&config, &args[1], fps) {
                Ok(result) => result,
                Err(err) => {
                    eprintln!("Error: {}", err);
                    std::process::exit(1);
                }
            }
        }
        _ => {
            print_usage(&args[0], &config);
            std::process::exit(1);
        }
    };

    let frame_interval = create_frame_interval(fps);
    let mut app = OverlayApplication::new(media_source, frame_interval);
    app.run()?;

    Ok(())
}

/// Handle the case when no command line arguments are provided
fn handle_no_arguments(config: &Option<Config>) -> Result<(MediaSource, u64)> {
    if let Some(cfg) = config {
        if let Some(default_preset) = cfg.get_default() {
            log::info!(
                "Using default preset: {} (fps: {})",
                default_preset.path,
                default_preset.fps.unwrap_or(30)
            );

            let media_source = create_media_source_from_preset(default_preset)?;
            Ok((media_source, default_preset.fps.unwrap_or(30)))
        } else {
            Err(anyhow!("No default preset configured"))
        }
    } else {
        Err(anyhow!("No config file found and no arguments provided"))
    }
}

/// Handle the case when a single argument is provided (path or preset name)
fn handle_single_argument(config: &Option<Config>, arg: &str) -> Result<(MediaSource, u64)> {
    if is_likely_path(arg) {
        // Treat as path
        let media_source = create_media_source_from_path(arg)?;
        log::info!("Using path: {}", arg);
        Ok((media_source, 30))
    } else {
        // Try as preset first, then as path
        handle_preset_or_path(config, arg, 30)
    }
}

/// Handle the case when two arguments are provided (path/preset and fps)
fn handle_argument_with_fps(
    config: &Option<Config>,
    arg: &str,
    fps: u64,
) -> Result<(MediaSource, u64)> {
    if is_likely_path(arg) {
        // Treat as path
        let media_source = create_media_source_from_path(arg)?;
        log::info!("Using path: {} with FPS: {}", arg, fps);
        Ok((media_source, fps))
    } else {
        // Try as preset first, then as path
        handle_preset_or_path(config, arg, fps)
    }
}

/// Try to resolve an argument as a preset first, then as a path
fn handle_preset_or_path(
    config: &Option<Config>,
    arg: &str,
    fps_override: u64,
) -> Result<(MediaSource, u64)> {
    if let Some(cfg) = config {
        if let Some(preset) = cfg.get_preset(arg) {
            let actual_fps = if fps_override == 30 {
                preset.fps.unwrap_or(30)
            } else {
                fps_override
            };

            if fps_override == 30 {
                log::info!(
                    "Using preset '{}': {} (fps: {})",
                    arg,
                    preset.path,
                    actual_fps
                );
            } else {
                log::info!(
                    "Using preset '{}': {} with FPS override: {}",
                    arg,
                    preset.path,
                    fps_override
                );
            }

            let media_source = create_media_source_from_preset(preset)?;
            Ok((media_source, actual_fps))
        } else {
            // Not a preset, try as directory path
            try_path_fallback(cfg, arg, fps_override)
        }
    } else {
        // No config file, treat as directory path
        let media_source = create_media_source_from_path(arg)?;
        if fps_override == 30 {
            log::info!("Using directory: {}", arg);
        } else {
            log::info!("Using directory: {} with FPS: {}", arg, fps_override);
        }
        Ok((media_source, fps_override))
    }
}

/// Try to use the argument as a path when it's not found as a preset
fn try_path_fallback(config: &Config, arg: &str, fps: u64) -> Result<(MediaSource, u64)> {
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
    if fps == 30 {
        log::info!("Using directory: {}", arg);
    } else {
        log::info!("Using directory: {} with FPS: {}", arg, fps);
    }
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

/// Parse FPS from a string, with fallback to default value
fn parse_fps(fps_str: &str) -> u64 {
    fps_str.parse::<u64>().unwrap_or_else(|_| {
        log::warn!("Invalid FPS value '{}', using default 30", fps_str);
        30
    })
}

/// Create a Duration for the frame interval based on FPS
fn create_frame_interval(fps: u64) -> Duration {
    if fps > 0 {
        Duration::from_secs_f64(1.0 / fps as f64)
    } else {
        Duration::from_millis(33)
    }
}

fn print_usage(program_name: &str, config: &Option<Config>) {
    eprintln!("Usage: {} [path|preset] [fps]", program_name);
    eprintln!("  path: Directory with images, GIF file, or APNG file");
    eprintln!("  preset: Named preset from config file");
    eprintln!("  fps: Frames per second (default: 30, or from preset)");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {program_name} ./frames         # Use frames directory");
    eprintln!("  {program_name} animation.gif    # Use GIF file");
    eprintln!("  {program_name} konata           # Use 'konata' preset");
    eprintln!("  {program_name} 1                # Use preset '1'");
    eprintln!("  {program_name} ./frames 60      # Use frames directory at 60 FPS");
    eprintln!();

    if let Some(config) = config {
        let presets = config.list_presets();
        if !presets.is_empty() {
            eprintln!("Available presets: {}", presets.join(", "));
        } else {
            eprintln!("No presets configured.");
        }
    } else {
        eprintln!(
            "No config file found. Create ~/.config/konata-dance/config.toml to use presets."
        );
    }

    eprintln!();
    eprintln!("Config file format (~/.config/konata-dance/config.toml):");
    eprintln!("[default]");
    eprintln!("path = \"/path/to/default/animation\"");
    eprintln!("fps = 30");
    eprintln!();
    eprintln!("[konata]");
    eprintln!("path = \"/path/to/konata/frames\"");
    eprintln!("fps = 24");
    eprintln!();
    eprintln!("[1]");
    eprintln!("path = \"/path/to/animation1.gif\"");
    eprintln!("fps = 60");
}
