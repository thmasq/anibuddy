use anyhow::{Result, anyhow};
use glob::glob;
use image::{ImageFormat, RgbaImage};
use include_dir::{Dir, File};
use std::path::{Path, PathBuf};

pub struct ImageSequence {
    images: Vec<RgbaImage>,
    current_index: usize,
}

impl ImageSequence {
    pub fn load(directory: &Path) -> Result<Self> {
        let pattern = directory.join("*.png").to_string_lossy().to_string();
        log::info!("Looking for PNGs in pattern: {}", pattern);

        let mut image_paths: Vec<PathBuf> = glob(&pattern)?.filter_map(Result::ok).collect();

        image_paths.sort();

        if image_paths.is_empty() {
            return Err(anyhow!("No PNG files found in {}", directory.display()));
        }

        log::info!("Found {} images in sequence", image_paths.len());

        let mut images = Vec::with_capacity(image_paths.len());
        for path in image_paths {
            log::info!("Loading {}", path.display());
            let img = image::open(&path)?.to_rgba8();
            images.push(img);
        }

        Ok(Self {
            images,
            current_index: 0,
        })
    }

    pub fn load_embedded(dir: &'static Dir) -> Result<Self> {
        let mut image_files: Vec<&File> = dir
            .files()
            .filter(|file| {
                file.path()
                    .extension()
                    .map_or(false, |ext| ext.to_string_lossy().to_lowercase() == "png")
            })
            .collect();

        image_files.sort_by(|a, b| a.path().file_name().cmp(&b.path().file_name()));

        if image_files.is_empty() {
            return Err(anyhow!("No PNG files found in embedded directory"));
        }

        log::info!("Found {} embedded images in sequence", image_files.len());

        let mut images = Vec::with_capacity(image_files.len());
        for file in image_files {
            log::info!("Loading embedded file: {}", file.path().display());
            let data = file.contents();
            let img = image::load_from_memory_with_format(data, ImageFormat::Png)?.to_rgba8();
            images.push(img);
        }

        Ok(Self {
            images,
            current_index: 0,
        })
    }

    pub fn current_image(&self) -> Option<&RgbaImage> {
        self.images.get(self.current_index)
    }

    pub fn advance(&mut self) {
        self.current_index = (self.current_index + 1) % self.images.len();
    }

    pub fn count(&self) -> usize {
        self.images.len()
    }
}
