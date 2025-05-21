use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::image_loader::ImageSequence;
use crate::renderer::Renderer;

pub struct OverlayApplication {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    image_sequence: Option<ImageSequence>,
    image_directory: PathBuf,
    last_frame_time: Instant,
    frame_interval: Duration,
}

impl OverlayApplication {
    pub fn new(image_directory: PathBuf) -> Self {
        Self {
            window: None,
            renderer: None,
            image_sequence: None,
            image_directory,
            last_frame_time: Instant::now(),
            frame_interval: Duration::from_millis(33), // ~30 FPS
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let event_loop = EventLoop::new()?;

        self.image_sequence = Some(ImageSequence::load(&self.image_directory)?);

        if let Some(sequence) = &self.image_sequence {
            log::info!("Loaded {} images in sequence", sequence.count());
        } else {
            log::error!("Failed to load image sequence");
            return Ok(());
        }

        event_loop.run_app(self)?;

        Ok(())
    }

    fn update(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_frame_time) >= self.frame_interval {
            self.last_frame_time = now;

            if let Some(sequence) = &mut self.image_sequence {
                sequence.advance();

                if let (Some(renderer), Some(image)) =
                    (&mut self.renderer, sequence.current_image())
                {
                    renderer.load_image(image);
                }
            }
        }
    }

    fn render(&mut self) -> Result<()> {
        if let Some(renderer) = &mut self.renderer {
            renderer.render()?;
        }

        Ok(())
    }
}

impl ApplicationHandler for OverlayApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = WindowAttributes::default()
            .with_title("PNG Overlay")
            .with_transparent(true)
            .with_decorations(false)
            .with_resizable(false);

        match event_loop.create_window(window_attributes) {
            Ok(window) => {
                let window_arc = Arc::new(window);
                self.window = Some(window_arc.clone());

                pollster::block_on(async {
                    match Renderer::new(window_arc).await {
                        Ok(renderer) => {
                            self.renderer = Some(renderer);

                            if let (Some(renderer), Some(sequence)) =
                                (&mut self.renderer, &self.image_sequence)
                            {
                                if let Some(image) = sequence.current_image() {
                                    renderer.load_image(image);
                                }
                            }
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
                event_loop.exit();
            }
            winit::event::WindowEvent::Resized(size) => {
                log::info!("Window resized to {}x{}", size.width, size.height);
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }
            winit::event::WindowEvent::RedrawRequested => {
                self.update();

                if let Err(err) = self.render() {
                    log::error!("Render error: {}", err);
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if now.duration_since(self.last_frame_time) >= self.frame_interval {
            if let Some(window) = &self.window {
                window.request_redraw();
                event_loop.set_control_flow(ControlFlow::WaitUntil(now + self.frame_interval));
            }
        }
    }
}
