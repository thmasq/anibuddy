use anyhow::{Result, anyhow};
use include_dir::Dir;
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor,
    delegate_layer,
    delegate_output,
    delegate_registry,
    delegate_seat,
    output::{OutputHandler, OutputState},
    // Use the reexported calloop from smithay_client_toolkit
    reexports::calloop::EventLoop,
    reexports::calloop_wayland_source::WaylandSource,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{Capability, SeatHandler, SeatState},
    shell::wlr_layer::LayerShellHandler,
    shell::wlr_layer::{
        Anchor, KeyboardInteractivity, Layer, LayerShell, LayerSurface, LayerSurfaceConfigure,
    },
};
use std::path::PathBuf;
use std::ptr::NonNull;
use std::time::{Duration, Instant};
use wayland_client::Proxy;
use wayland_client::globals::registry_queue_init;
use wayland_client::{
    Connection, QueueHandle,
    protocol::{wl_output, wl_seat, wl_surface},
};

use crate::image_loader::ImageSequence;
use crate::renderer::Renderer;

pub struct OverlayApplication {
    image_directory: Option<PathBuf>,
    embedded_dir: Option<&'static Dir<'static>>,
    frame_interval: Duration,
}

#[allow(dead_code)]
struct AppState {
    registry_state: RegistryState,
    compositor_state: CompositorState,
    output_state: OutputState,
    seat_state: SeatState,
    layer_shell: LayerShell,

    surface: Option<wl_surface::WlSurface>,
    layer_surface: Option<LayerSurface>,

    renderer: Option<Renderer>,
    image_sequence: Option<ImageSequence>,

    last_frame_time: Instant,
    frame_interval: Duration,
    current_frame_index: usize,
    frame_count: usize,
    exit: bool,

    width: u32,
    height: u32,
    connection: Connection,

    first_frame_requested: bool,
}

impl CompositorHandler for AppState {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        // Handle scale factor changes if needed
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
        // Handle transform changes if needed
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        self.update();

        if let Err(err) = self.render() {
            log::error!("Render error: {}", err);
        }

        // Request next frame
        if let Some(surface) = &self.surface {
            surface.frame(qh, surface.clone());
            surface.commit();
        }
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        // Handle surface enter
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        // Handle surface leave
    }
}

impl OutputHandler for AppState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        // Handle new output
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        // Handle output update
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        // Handle output destroyed
    }
}

impl SeatHandler for AppState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        _capability: Capability,
    ) {
        // Handle new capability
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        _capability: Capability,
    ) {
        // Handle capability removal
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl LayerShellHandler for AppState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        log::info!("Layer surface configured: {:?}", configure);

        self.width = configure.new_size.0;
        self.height = configure.new_size.1;

        // Update renderer size if available
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(self.width, self.height);
        } else {
            // Initialize renderer on first configure
            self.initialize_renderer();
        }

        // Request the first frame after configuration
        if !self.first_frame_requested {
            if let Some(surface) = &self.surface {
                surface.frame(qh, surface.clone());
                surface.commit();
                self.first_frame_requested = true;
            }
        }
    }
}

impl ProvidesRegistryState for AppState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState, SeatState];
}

// Implement all the delegate macros
delegate_compositor!(AppState);
delegate_output!(AppState);
delegate_seat!(AppState);
delegate_layer!(AppState);
delegate_registry!(AppState);

impl AppState {
    fn update(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_frame_time) >= self.frame_interval {
            self.last_frame_time = now;

            if self.frame_count > 0 {
                self.current_frame_index = (self.current_frame_index + 1) % self.frame_count;

                if let Some(renderer) = &mut self.renderer {
                    renderer.set_current_texture_index(self.current_frame_index);
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

    fn initialize_renderer(&mut self) {
        if self.renderer.is_some() || self.surface.is_none() {
            return;
        }

        // Create WGPU surface from Wayland surface
        if let Some(surface) = &self.surface {
            let raw_display_handle = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
                NonNull::new(self.connection.backend().display_ptr() as *mut _).unwrap(),
            ));

            let raw_window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(
                NonNull::new(surface.id().as_ptr() as *mut _).unwrap(),
            ));

            // Initialize WGPU renderer
            pollster::block_on(async {
                match Renderer::new_from_raw_handle(raw_display_handle, raw_window_handle).await {
                    Ok(mut renderer) => {
                        // Set initial size
                        renderer.resize(self.width, self.height);

                        // Preload images if sequence is available
                        if let Some(sequence) = &self.image_sequence {
                            let all_images = sequence.get_all_images();
                            renderer.preload_images(&all_images);
                            log::info!("Preloaded {} images to GPU memory", all_images.len());
                        }

                        self.renderer = Some(renderer);
                    }
                    Err(err) => {
                        log::error!("Failed to create renderer: {}", err);
                    }
                }
            });
        }
    }
}

impl OverlayApplication {
    pub fn new_embedded(dir: &'static Dir, frame_interval: Duration) -> Self {
        Self {
            image_directory: None,
            embedded_dir: Some(dir),
            frame_interval,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        // Connect to Wayland compositor
        let connection = Connection::connect_to_env()?;

        // Setup event loop
        let mut event_loop: EventLoop<AppState> = EventLoop::try_new()?;
        let loop_handle = event_loop.handle();

        // Initialize Wayland registry
        let (globals, event_queue) = registry_queue_init(&connection)?;
        let qh = event_queue.handle();

        // Create Wayland event source and insert it into the event loop
        WaylandSource::new(connection.clone(), event_queue)
            .insert(loop_handle.clone())
            .expect("Failed to insert Wayland source");

        // Initialize SCTK states
        let compositor_state =
            CompositorState::bind(&globals, &qh).expect("wl_compositor not available");

        let layer_shell = LayerShell::bind(&globals, &qh).expect("wlr_layer_shell not available");

        // Load image sequence
        let image_sequence = if let Some(dir) = self.image_directory.as_ref() {
            Some(ImageSequence::load(dir)?)
        } else if let Some(dir) = self.embedded_dir {
            Some(ImageSequence::load_embedded(dir)?)
        } else {
            return Err(anyhow!("No image source specified"));
        };

        let frame_count = image_sequence.as_ref().map_or(0, |s| s.count());
        log::info!("Loaded {} images in sequence", frame_count);

        // Get initial dimensions from first image
        let (width, height) = if let Some(sequence) = &image_sequence {
            if let Some(image) = sequence.current_image() {
                let dims = image.dimensions();
                (dims.0, dims.1)
            } else {
                (256, 256)
            }
        } else {
            (256, 256)
        };

        // Create surface
        let surface = compositor_state.create_surface(&qh);

        // Create layer surface
        let layer_surface = layer_shell.create_layer_surface(
            &qh,
            surface.clone(),
            Layer::Overlay,
            Some("konata-dance-rs"),
            None,
        );

        // Configure layer surface
        layer_surface.set_anchor(Anchor::TOP | Anchor::RIGHT);
        layer_surface.set_exclusive_zone(-1); // Don't reserve space
        layer_surface.set_size(width, height);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);

        // Initial commit to receive configure
        surface.commit();

        // Create state
        let mut app_state = AppState {
            registry_state: RegistryState::new(&globals),
            compositor_state,
            output_state: OutputState::new(&globals, &qh),
            seat_state: SeatState::new(&globals, &qh),
            layer_shell,

            surface: Some(surface),
            layer_surface: Some(layer_surface),

            renderer: None,
            image_sequence,

            last_frame_time: Instant::now(),
            frame_interval: self.frame_interval,
            current_frame_index: 0,
            frame_count,
            exit: false,

            width,
            height,
            connection,
            first_frame_requested: false,
        };

        // Run event loop
        while !app_state.exit {
            event_loop.dispatch(Duration::from_millis(16), &mut app_state)?;
        }

        Ok(())
    }
}
