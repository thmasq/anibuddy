# anibuddy

An animated overlay application for Wayland desktops. Display animated GIFs, APNGs, or image sequences as desktop overlays with GPU acceleration and optional delta compression.

## Installation

### Arch Linux

```bash
git clone https://github.com/thmasq/anibuddy.git
cd anibuddy
makepkg -si
```

### Dependencies

**Required:**
- Wayland compositor
- Vulkan drivers for your GPU:
  - Intel: `vulkan-intel`
  - AMD: `vulkan-radeon` 
  - NVIDIA: `nvidia-utils`

## Usage

### Basic Usage

```bash
# Use a directory of images
anibuddy ./frames

# Use a GIF file
anibuddy animation.gif

# Use an APNG file  
anibuddy animation.apng

# Control frame rate
anibuddy ./frames --fps 60

# Enable delta compression (reduces memory usage)
anibuddy --compress ./frames
```

### Configuration

Create `~/.config/anibuddy/config.toml`:

```toml
# Default preset (used when no arguments provided)
[default]
path = "/path/to/your/default/animation"
fps = 30
compress = false

# Named presets
[konata]
path = "/path/to/konata/frames"  
fps = 24
compress = true

[dancing]
path = "/path/to/dancing.gif"
fps = 60
```

### Using Presets

```bash
# List available presets
anibuddy --list-presets

# Use a named preset
anibuddy konata

# Use preset with overrides
anibuddy konata --fps 30 --compress
```

## Features

- **Multiple formats**: Directories of images, GIF, APNG
- **Delta compression**: Reduces memory usage by 50-90% for animations with small frame changes
- **GPU accelerated**: Uses Vulkan/wgpu for efficient rendering
- **Transparent overlay**: Renders on top of other applications
- **Wayland native**: Designed specifically for Wayland compositors

## Controls

- Close the overlay window to exit
- Frame timing is controlled by FPS setting

## Supported Image Formats

- PNG, JPG, JPEG (in directories)
- Animated GIF
- Animated PNG (APNG)
