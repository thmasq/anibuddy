use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::media_loader::{MediaSequence, MediaSource};
use crate::renderer::Renderer;

pub struct OverlayApplication {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    media_sequence: Option<MediaSequence>,
    media_source: Option<MediaSource>,
    last_frame_time: Instant,
    frame_interval: Duration,
    current_frame_index: usize,
    frame_count: usize,
    use_compression: bool,
    frame_update_in_progress: bool,
    is_shutting_down: bool,
}

impl OverlayApplication {
    pub fn new(source: MediaSource, frame_interval: Duration, use_compression: bool) -> Self {
        Self {
            window: None,
            renderer: None,
            media_sequence: None,
            media_source: Some(source),
            last_frame_time: Instant::now(),
            frame_interval,
            current_frame_index: 0,
            frame_count: 0,
            use_compression,
            frame_update_in_progress: false,
            is_shutting_down: false,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let event_loop = EventLoop::new()?;

        // Load the media sequence
        if let Some(source) = self.media_source.take() {
            self.media_sequence = Some(MediaSequence::load(source)?);
        } else {
            return Err(anyhow::format_err!("No media source specified"));
        };

        if let Some(sequence) = &self.media_sequence {
            self.frame_count = sequence.count();
            log::info!("Loaded {} frames in sequence", self.frame_count);
        } else {
            log::error!("Failed to load media sequence");
            return Ok(());
        }

        event_loop.run_app(self)?;

        Ok(())
    }

    /// Cleanup resources before shutdown
    fn cleanup(&mut self) {
        if self.is_shutting_down {
            return;
        }

        log::info!("Starting application cleanup");
        self.is_shutting_down = true;

        if let Some(mut renderer) = self.renderer.take() {
            renderer.cleanup();
        }

        self.media_sequence = None;
        self.window = None;

        log::info!("Application cleanup complete");
    }

    fn update(&mut self) {
        if self.is_shutting_down {
            return;
        }

        let now = Instant::now();
        if now.duration_since(self.last_frame_time) >= self.frame_interval
            && !self.frame_update_in_progress
        {
            self.last_frame_time = now;

            if self.frame_count > 0 {
                let new_frame_index = (self.current_frame_index + 1) % self.frame_count;

                if let Some(renderer) = &mut self.renderer {
                    self.frame_update_in_progress = true;

                    // For compressed sequences, we need to handle async frame reconstruction
                    if self.use_compression {
                        // Create a future to update the frame
                        let renderer_ptr = renderer as *mut Renderer;
                        let frame_index = new_frame_index;

                        // This could probably be done better
                        // For now, we'll use pollster to block on the async operation
                        match pollster::block_on(async {
                            let renderer = unsafe { &mut *renderer_ptr };
                            renderer.set_current_texture_index(frame_index).await
                        }) {
                            Ok(_) => {
                                self.current_frame_index = new_frame_index;
                            }
                            Err(e) => {
                                log::error!("Failed to update compressed frame: {}", e);
                            }
                        }
                    } else {
                        // For uncompressed sequences, this is synchronous
                        match pollster::block_on(
                            renderer.set_current_texture_index(new_frame_index),
                        ) {
                            Ok(_) => {
                                self.current_frame_index = new_frame_index;
                            }
                            Err(e) => {
                                log::error!("Failed to update frame: {}", e);
                            }
                        }
                    }

                    self.frame_update_in_progress = false;
                }
            }
        }
    }

    fn render(&mut self) -> Result<()> {
        if self.is_shutting_down {
            return Ok(());
        }

        if let Some(renderer) = &mut self.renderer {
            renderer.render()?;
        }

        Ok(())
    }
}

impl ApplicationHandler for OverlayApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let (width, height) = if let Some(sequence) = &self.media_sequence {
            if let Some(image) = sequence.current_image() {
                let dimensions = image.dimensions();
                log::info!(
                    "Using image dimensions for window: {}x{}",
                    dimensions.0,
                    dimensions.1
                );
                (dimensions.0, dimensions.1)
            } else {
                log::info!("No image found, using default dimensions");
                (800, 600)
            }
        } else {
            log::info!("No media sequence found, using default dimensions");
            (800, 600)
        };

        let window_attributes = WindowAttributes::default()
            .with_title(if self.use_compression {
                "PNG Overlay (Delta Compressed)"
            } else {
                "PNG Overlay"
            })
            .with_transparent(true)
            .with_decorations(false)
            .with_resizable(false)
            .with_inner_size(PhysicalSize::new(width, height));

        match event_loop.create_window(window_attributes) {
            Ok(window) => {
                let window_arc = Arc::new(window);
                self.window = Some(window_arc.clone());

                pollster::block_on(async {
                    match Renderer::new(window_arc).await {
                        Ok(mut renderer) => {
                            if let Some(sequence) = &self.media_sequence {
                                let all_images = sequence.get_all_images();

                                if self.use_compression {
                                    log::info!(
                                        "Loading {} images with delta compression",
                                        all_images.len()
                                    );
                                    match renderer.preload_images_compressed(&all_images).await {
                                        Ok(_) => {
                                            log::info!("Successfully loaded compressed sequence");
                                        }
                                        Err(e) => {
                                            log::error!(
                                                "Failed to load compressed sequence: {}, falling back to uncompressed",
                                                e
                                            );
                                            renderer.preload_images(&all_images);
                                        }
                                    }
                                } else {
                                    log::info!(
                                        "Loading {} images without compression",
                                        all_images.len()
                                    );
                                    renderer.preload_images(&all_images);
                                }
                            }

                            self.renderer = Some(renderer);
                        }
                        Err(err) => {
                            log::error!("Failed to create renderer: {}", err);
                            event_loop.exit();
                        }
                    }
                });
            }
            Err(err) => {
                log::error!("Failed to create window: {}", err);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            winit::event::WindowEvent::CloseRequested => {
                log::info!("Window close requested");

                self.cleanup();

                event_loop.exit();
            }
            winit::event::WindowEvent::Resized(size) => {
                log::info!("Window resized to {}x{}", size.width, size.height);
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }
            winit::event::WindowEvent::RedrawRequested => {
                if !self.is_shutting_down {
                    self.update();

                    if let Err(err) = self.render() {
                        log::error!("Render error: {}", err);
                    }

                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.is_shutting_down {
            return;
        }

        let now = Instant::now();
        if now.duration_since(self.last_frame_time) >= self.frame_interval {
            if let Some(window) = &self.window {
                window.request_redraw();
                event_loop.set_control_flow(ControlFlow::WaitUntil(now + self.frame_interval));
            }
        }
    }
}

impl Drop for OverlayApplication {
    fn drop(&mut self) {
        log::debug!("Dropping OverlayApplication");
        self.cleanup();
    }
}
