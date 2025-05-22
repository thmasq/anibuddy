use anyhow::{Result, anyhow};
use glob::glob;
use image::RgbaImage;
use std::fs::File as StdFile;
use std::path::{Path, PathBuf};

pub enum MediaSource {
    Directory(PathBuf),
    GifFile(PathBuf),
    ApngFile(PathBuf),
}

pub struct MediaSequence {
    images: Vec<RgbaImage>,
    current_index: usize,
}

impl MediaSequence {
    pub fn load(source: MediaSource) -> Result<Self> {
        let images = match source {
            MediaSource::Directory(path) => Self::load_image_directory(&path)?,
            MediaSource::GifFile(path) => Self::load_gif(&path)?,
            MediaSource::ApngFile(path) => Self::load_apng(&path)?,
        };

        if images.is_empty() {
            return Err(anyhow!("No images loaded from source"));
        }

        Ok(Self {
            images,
            current_index: 0,
        })
    }

    fn load_image_directory(directory: &Path) -> Result<Vec<RgbaImage>> {
        let patterns = ["*.png", "*.jpg", "*.jpeg"];
        let mut image_paths = Vec::new();

        for pattern in &patterns {
            let full_pattern = directory.join(pattern).to_string_lossy().to_string();
            let paths: Vec<PathBuf> = glob(&full_pattern)?.filter_map(Result::ok).collect();
            image_paths.extend(paths);
        }

        image_paths.sort();

        if image_paths.is_empty() {
            return Err(anyhow!("No image files found in {}", directory.display()));
        }

        log::info!("Found {} images in directory", image_paths.len());

        let mut images = Vec::with_capacity(image_paths.len());
        for path in image_paths {
            log::debug!("Loading {}", path.display());
            let img = image::open(&path)?.to_rgba8();
            images.push(img);
        }

        Ok(images)
    }

    fn load_gif(path: &Path) -> Result<Vec<RgbaImage>> {
        log::info!("Loading GIF file: {}", path.display());

        let file = StdFile::open(path)?;
        let mut decoder = gif::DecodeOptions::new();
        decoder.set_color_output(gif::ColorOutput::RGBA);

        let mut decoder = decoder
            .read_info(file)
            .map_err(|e| anyhow!("Failed to read GIF info: {}", e))?;

        let mut images = Vec::new();

        while let Some(frame) = decoder
            .read_next_frame()
            .map_err(|e| anyhow!("Failed to read GIF frame: {}", e))?
        {
            let width = frame.width as u32;
            let height = frame.height as u32;

            // Convert the frame buffer to RgbaImage
            let rgba_image = RgbaImage::from_raw(width, height, frame.buffer.to_vec())
                .ok_or_else(|| anyhow!("Failed to create image from GIF frame"))?;

            images.push(rgba_image);
        }

        log::info!("Loaded {} frames from GIF", images.len());
        Ok(images)
    }

    fn load_apng(path: &Path) -> Result<Vec<RgbaImage>> {
        log::info!("Loading APNG file: {}", path.display());

        let file = StdFile::open(path)?;
        let decoder = png::Decoder::new(file);
        let mut reader = decoder
            .read_info()
            .map_err(|e| anyhow!("Failed to read PNG info: {}", e))?;

        let mut images = Vec::new();

        // Check if it's animated
        if let Some(animation_control) = reader.info().animation_control() {
            log::info!("APNG has {} frames", animation_control.num_frames);

            let buffer_size = reader.output_buffer_size();
            let mut buffer = vec![0; buffer_size];

            // Read all frames
            loop {
                match reader.next_frame(&mut buffer) {
                    Ok(output_info) => {
                        // process frame
                        let width = output_info.width;
                        let height = output_info.height;

                        let rgba_buffer = match output_info.color_type {
                            png::ColorType::Rgba => buffer.clone(),
                            png::ColorType::Rgb => {
                                let mut rgba = Vec::with_capacity((width * height * 4) as usize);
                                for chunk in buffer.chunks(3) {
                                    rgba.extend_from_slice(chunk);
                                    rgba.push(255);
                                }
                                rgba
                            }
                            _ => {
                                return Err(anyhow!(
                                    "Unsupported PNG color type: {:?}",
                                    output_info.color_type
                                ));
                            }
                        };

                        let rgba_image = RgbaImage::from_raw(width, height, rgba_buffer)
                            .ok_or_else(|| anyhow!("Failed to create image from APNG frame"))?;

                        images.push(rgba_image);
                    }
                    Err(e) if format!("{}", e).contains("End of image has been reached") => {
                        // Gracefully end loop
                        break;
                    }
                    Err(png::DecodingError::IoError(ref io_err))
                        if io_err.kind() == std::io::ErrorKind::UnexpectedEof =>
                    {
                        break;
                    }
                    Err(e) => return Err(anyhow!("Error reading APNG frame: {}", e)),
                }
            }
        } else {
            // Not animated, just load as single image
            log::info!("PNG is not animated, loading as single frame");
            let img = image::open(path)?.to_rgba8();
            images.push(img);
        }

        log::info!("Loaded {} frames from APNG", images.len());
        Ok(images)
    }

    pub fn current_image(&self) -> Option<&RgbaImage> {
        self.images.get(self.current_index)
    }

    pub fn count(&self) -> usize {
        self.images.len()
    }

    pub fn get_all_images(&self) -> &[RgbaImage] {
        &self.images
    }
}

// Helper function to detect media type from path
pub fn detect_media_type(path: &Path) -> Result<MediaSource> {
    if path.is_dir() {
        Ok(MediaSource::Directory(path.to_path_buf()))
    } else if path.is_file() {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_lowercase());

        match extension.as_deref() {
            Some("gif") => Ok(MediaSource::GifFile(path.to_path_buf())),
            Some("png") => {
                // Check if it's an APNG by reading the file
                if is_apng(path)? {
                    Ok(MediaSource::ApngFile(path.to_path_buf()))
                } else {
                    // Single PNG, treat as directory with one file
                    let parent = path
                        .parent()
                        .ok_or_else(|| anyhow!("Cannot get parent directory"))?;
                    Ok(MediaSource::Directory(parent.to_path_buf()))
                }
            }
            Some("jpg") | Some("jpeg") => {
                // Single image, treat as directory
                let parent = path
                    .parent()
                    .ok_or_else(|| anyhow!("Cannot get parent directory"))?;
                Ok(MediaSource::Directory(parent.to_path_buf()))
            }
            _ => Err(anyhow!("Unsupported file type: {:?}", extension)),
        }
    } else {
        Err(anyhow!("Path does not exist: {}", path.display()))
    }
}

fn is_apng(path: &Path) -> Result<bool> {
    let file = StdFile::open(path)?;
    let decoder = png::Decoder::new(file);
    let reader = decoder.read_info()?;
    Ok(reader.info().animation_control().is_some())
}
