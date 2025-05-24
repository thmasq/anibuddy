use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use image::RgbaImage;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::delta_compression::{CompressedSequence, DeltaCompressor};

const VERTEX_SHADER: &str = r#"
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    // Quad vertices (triangle strip): full screen
    var positions = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, 1.0)
    );
    
    var texcoords = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0)
    );
    
    return vec4<f32>(positions[vertex_index], 0.0, 1.0);
}
"#;

const FRAGMENT_SHADER: &str = r#"
@group(0) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;
@group(0) @binding(2)
var<uniform> dimensions: vec4<f32>; // window_width, window_height, image_width, image_height

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    // Calculate texture coordinates based on actual dimensions
    let tex_coords = vec2<f32>(
        pos.x / dimensions.x,
        pos.y / dimensions.y
    );
    
    // Sample the texture
    return textureSample(t_diffuse, s_diffuse, tex_coords);
}
"#;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Dimensions {
    window_width: f32,
    window_height: f32,
    image_width: f32,
    image_height: f32,
}

pub enum SequenceType {
    Uncompressed {
        texture_bind_groups: Vec<wgpu::BindGroup>,
    },
    Compressed {
        compressed_sequence: CompressedSequence,
        current_frame_texture: wgpu::Texture,
        current_frame_bind_group: wgpu::BindGroup,
        reconstructed_frame: Option<RgbaImage>,
    },
}

pub struct Renderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sequence_type: Option<SequenceType>,
    current_texture_index: usize,
    config: wgpu::SurfaceConfiguration,
    dimensions_buffer: wgpu::Buffer,
    current_dimensions: Dimensions,

    delta_compressor: Option<DeltaCompressor>,
    sampler: wgpu::Sampler,
}

impl Renderer {
    pub async fn new(window: Arc<Window>) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .map_err(|_| anyhow::anyhow!("Failed to find an appropriate adapter"))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Overlay Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let device_arc = Arc::new(device);
        let queue_arc = Arc::new(queue);

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
            view_formats: vec![],
        };

        surface.configure(&device_arc, &config);

        // Initialize the dimensions
        let current_dimensions = Dimensions {
            window_width: size.width as f32,
            window_height: size.height as f32,
            image_width: size.width as f32,
            image_height: size.height as f32,
        };

        // Create dimensions buffer
        let dimensions_buffer = device_arc.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Dimensions Buffer"),
            contents: bytemuck::cast_slice(&[current_dimensions]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout =
            device_arc.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device_arc.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_shader = device_arc.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Vertex Shader"),
            source: wgpu::ShaderSource::Wgsl(VERTEX_SHADER.into()),
        });

        let fragment_shader = device_arc.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(FRAGMENT_SHADER.into()),
        });

        let pipeline = device_arc.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &fragment_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview: None,
            cache: None,
        });

        // Create reusable sampler
        let sampler = device_arc.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Initialize delta compressor
        let delta_compressor = Some(DeltaCompressor::new(device_arc.clone(), queue_arc.clone())?);

        Ok(Self {
            device: device_arc,
            queue: queue_arc,
            surface,
            pipeline,
            bind_group_layout,
            sequence_type: None,
            current_texture_index: 0,
            config,
            dimensions_buffer,
            current_dimensions,
            delta_compressor,
            sampler,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);

        // Update dimensions
        self.current_dimensions.window_width = width as f32;
        self.current_dimensions.window_height = height as f32;

        // Update the buffer
        self.queue.write_buffer(
            &self.dimensions_buffer,
            0,
            bytemuck::cast_slice(&[self.current_dimensions]),
        );

        log::info!("Resized to {}x{}", width, height);
    }

    // New method to preload all images at once
    pub fn preload_images(&mut self, images: &[RgbaImage]) {
        if images.is_empty() {
            log::warn!("No images to preload");
            return;
        }

        // Clear any existing sequence
        self.sequence_type = None;

        // Use first image dimensions for the window
        let first_dims = images[0].dimensions();
        self.current_dimensions.image_width = first_dims.0 as f32;
        self.current_dimensions.image_height = first_dims.1 as f32;

        // Update the dimensions buffer
        self.queue.write_buffer(
            &self.dimensions_buffer,
            0,
            bytemuck::cast_slice(&[self.current_dimensions]),
        );

        log::info!(
            "Preloading {} images to GPU memory (uncompressed)",
            images.len()
        );

        let mut texture_bind_groups = Vec::new();

        for (i, image) in images.iter().enumerate() {
            let dimensions = image.dimensions();

            let texture_size = wgpu::Extent3d {
                width: dimensions.0,
                height: dimensions.1,
                depth_or_array_layers: 1,
            };

            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("Image Texture {}", i)),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                image,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * dimensions.0),
                    rows_per_image: Some(dimensions.1),
                },
                texture_size,
            );

            let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Texture Bind Group {}", i)),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.dimensions_buffer.as_entire_binding(),
                    },
                ],
            });

            texture_bind_groups.push(bind_group);
        }

        self.sequence_type = Some(SequenceType::Uncompressed {
            texture_bind_groups,
        });

        self.current_texture_index = 0;
        log::info!(
            "Preloaded {} images to GPU memory (uncompressed)",
            images.len()
        );
    }

    pub async fn preload_images_compressed(&mut self, images: &[RgbaImage]) -> Result<()> {
        if images.is_empty() {
            log::warn!("No images to compress");
            return Ok(());
        }

        // Calculate original memory usage
        let original_size: usize = images.iter().map(|img| img.as_raw().len()).sum();
        log::info!(
            "Original sequence size: {:.2} MB",
            original_size as f64 / (1024.0 * 1024.0)
        );

        // Clear any existing sequence
        self.sequence_type = None;

        // Use first image dimensions for the window
        let first_dims = images[0].dimensions();
        self.current_dimensions.image_width = first_dims.0 as f32;
        self.current_dimensions.image_height = first_dims.1 as f32;

        // Update the dimensions buffer
        self.queue.write_buffer(
            &self.dimensions_buffer,
            0,
            bytemuck::cast_slice(&[self.current_dimensions]),
        );

        log::info!("Compressing {} images with delta compression", images.len());

        // Compress the sequence
        let compressed_sequence = if let Some(ref mut compressor) = self.delta_compressor {
            compressor.compress_sequence(images).await?
        } else {
            return Err(anyhow::anyhow!("Delta compressor not initialized"));
        };

        // Log compression statistics
        let compressed_size = compressed_sequence.memory_usage();
        let compression_ratio = compressed_sequence.compression_ratio(original_size);
        log::info!(
            "Compressed sequence size: {:.2} MB",
            compressed_size as f64 / (1024.0 * 1024.0)
        );
        log::info!("Compression ratio: {:.2}x", compression_ratio);

        // Create texture for current frame
        let texture_size = wgpu::Extent3d {
            width: first_dims.0,
            height: first_dims.1,
            depth_or_array_layers: 1,
        };

        let current_frame_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Current Frame Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload base frame initially
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &current_frame_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &compressed_sequence.base_frame,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * first_dims.0),
                rows_per_image: Some(first_dims.1),
            },
            texture_size,
        );

        let texture_view =
            current_frame_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let current_frame_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Current Frame Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.dimensions_buffer.as_entire_binding(),
                },
            ],
        });

        self.sequence_type = Some(SequenceType::Compressed {
            compressed_sequence,
            current_frame_texture,
            current_frame_bind_group,
            reconstructed_frame: Some(images[0].clone()),
        });

        self.current_texture_index = 0;
        log::info!("Successfully set up delta-compressed sequence");

        Ok(())
    }

    pub async fn set_current_texture_index(&mut self, index: usize) -> Result<()> {
        match &mut self.sequence_type {
            Some(SequenceType::Uncompressed {
                texture_bind_groups,
            }) => {
                if !texture_bind_groups.is_empty() {
                    self.current_texture_index = index % texture_bind_groups.len();
                }
            }
            Some(SequenceType::Compressed {
                compressed_sequence,
                current_frame_texture,
                reconstructed_frame,
                ..
            }) => {
                if index >= compressed_sequence.frame_count {
                    return Ok(());
                }

                self.current_texture_index = index;

                // Reconstruct the frame if it's not the base frame
                let new_frame = if index == 0 {
                    compressed_sequence.base_frame.clone()
                } else {
                    let delta_index = index - 1;
                    if delta_index < compressed_sequence.deltas.len() {
                        let base_frame = reconstructed_frame
                            .as_ref()
                            .ok_or_else(|| anyhow::anyhow!("No reconstructed frame available"))?;

                        if let Some(ref mut compressor) = self.delta_compressor {
                            compressor
                                .reconstruct_frame(
                                    base_frame,
                                    &compressed_sequence.deltas[delta_index],
                                )
                                .await?
                        } else {
                            return Err(anyhow::anyhow!("Delta compressor not available"));
                        }
                    } else {
                        return Ok(());
                    }
                };

                // Update the reconstructed frame for next iteration
                *reconstructed_frame = Some(new_frame.clone());

                // Upload the new frame to the current frame texture
                let (width, height) = new_frame.dimensions();
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: current_frame_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &new_frame,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * width),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
            }
            None => {}
        }

        Ok(())
    }

    pub fn render(&mut self) -> Result<()> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let bind_group = match &self.sequence_type {
            Some(SequenceType::Uncompressed {
                texture_bind_groups,
            }) => {
                if !texture_bind_groups.is_empty() {
                    Some(&texture_bind_groups[self.current_texture_index])
                } else {
                    None
                }
            }
            Some(SequenceType::Compressed {
                current_frame_bind_group,
                ..
            }) => Some(current_frame_bind_group),
            None => None,
        };

        if let Some(bind_group) = bind_group {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, bind_group, &[]);
            render_pass.draw(0..4, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        frame.present();

        Ok(())
    }
}
