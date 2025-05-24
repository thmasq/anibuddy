use anyhow::Result;
use image::RgbaImage;
use std::sync::Arc;

const DELTA_CALCULATE_SHADER: &str = r#"
@group(0) @binding(0)
var current_frame: texture_2d<f32>;
@group(0) @binding(1)
var previous_frame: texture_2d<f32>;
@group(0) @binding(2)
var delta_output: texture_storage_2d<rgba8sint, write>;

@compute @workgroup_size(8, 8)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(current_frame);
    let coords = vec2<i32>(i32(global_id.x), i32(global_id.y));
    
    if (coords.x >= i32(dims.x) || coords.y >= i32(dims.y)) {
        return;
    }
    
    let current_pixel = textureLoad(current_frame, coords, 0);
    let previous_pixel = textureLoad(previous_frame, coords, 0);
    
    // Calculate delta as signed difference
    let delta = current_pixel - previous_pixel;
    
    // Convert to signed integer format [-127, 127] range
    let delta_int = vec4<i32>(
        i32(clamp(delta.r * 127.0, -127.0, 127.0)),
        i32(clamp(delta.g * 127.0, -127.0, 127.0)),
        i32(clamp(delta.b * 127.0, -127.0, 127.0)),
        i32(clamp(delta.a * 127.0, -127.0, 127.0))
    );
    
    textureStore(delta_output, coords, delta_int);
}
"#;

const FRAME_RECONSTRUCT_SHADER: &str = r#"
@group(0) @binding(0)
var base_frame: texture_2d<f32>;
@group(0) @binding(1)
var delta_frame: texture_2d<i32>;
@group(0) @binding(2)
var output_frame: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(base_frame);
    let coords = vec2<i32>(i32(global_id.x), i32(global_id.y));
    
    if (coords.x >= i32(dims.x) || coords.y >= i32(dims.y)) {
        return;
    }
    
    let base_pixel = textureLoad(base_frame, coords, 0);
    let delta_pixel = textureLoad(delta_frame, coords, 0);
    
    // Convert delta back to float and apply to base
    let delta_float = vec4<f32>(
        f32(delta_pixel.r) / 127.0,
        f32(delta_pixel.g) / 127.0,
        f32(delta_pixel.b) / 127.0,
        f32(delta_pixel.a) / 127.0
    );
    
    let reconstructed = clamp(base_pixel + delta_float, vec4<f32>(0.0), vec4<f32>(1.0));
    textureStore(output_frame, coords, reconstructed);
}
"#;

pub struct DeltaFrame {
    pub data: Vec<i8>,
    pub width: u32,
    pub height: u32,
}

pub struct CompressedSequence {
    pub base_frame: RgbaImage,
    pub deltas: Vec<DeltaFrame>,
    pub frame_count: usize,
}

pub struct DeltaCompressor {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,

    // Delta calculation pipeline
    delta_pipeline: wgpu::ComputePipeline,
    delta_bind_group_layout: wgpu::BindGroupLayout,

    // Frame reconstruction pipeline
    reconstruct_pipeline: wgpu::ComputePipeline,
    reconstruct_bind_group_layout: wgpu::BindGroupLayout,

    // Working textures for computation
    working_texture_current: Option<wgpu::Texture>,
    working_texture_previous: Option<wgpu::Texture>,
    working_texture_delta: Option<wgpu::Texture>,
    working_texture_output: Option<wgpu::Texture>,

    // Buffer for reading back delta data
    staging_buffer: Option<wgpu::Buffer>,

    current_dimensions: (u32, u32),
}

impl DeltaCompressor {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Result<Self> {
        // Create delta calculation pipeline
        let delta_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Delta Calculate Shader"),
            source: wgpu::ShaderSource::Wgsl(DELTA_CALCULATE_SHADER.into()),
        });

        let delta_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Delta Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba8Sint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });

        let delta_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Delta Pipeline Layout"),
                bind_group_layouts: &[&delta_bind_group_layout],
                push_constant_ranges: &[],
            });

        let delta_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Delta Calculate Pipeline"),
            layout: Some(&delta_pipeline_layout),
            module: &delta_shader,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Create frame reconstruction pipeline
        let reconstruct_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Frame Reconstruct Shader"),
            source: wgpu::ShaderSource::Wgsl(FRAME_RECONSTRUCT_SHADER.into()),
        });

        let reconstruct_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Reconstruct Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Sint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });

        let reconstruct_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Reconstruct Pipeline Layout"),
                bind_group_layouts: &[&reconstruct_bind_group_layout],
                push_constant_ranges: &[],
            });

        let reconstruct_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Frame Reconstruct Pipeline"),
                layout: Some(&reconstruct_pipeline_layout),
                module: &reconstruct_shader,
                entry_point: Some("cs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });

        Ok(Self {
            device,
            queue,
            delta_pipeline,
            delta_bind_group_layout,
            reconstruct_pipeline,
            reconstruct_bind_group_layout,
            working_texture_current: None,
            working_texture_previous: None,
            working_texture_delta: None,
            working_texture_output: None,
            staging_buffer: None,
            current_dimensions: (0, 0),
        })
    }

    fn ensure_working_textures(&mut self, width: u32, height: u32) {
        if self.current_dimensions != (width, height) {
            log::info!("Creating working textures for {}x{}", width, height);

            // Current frame texture (input) - needs COPY_SRC since we copy from it to previous
            self.working_texture_current =
                Some(self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Working Texture Current"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_DST
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                }));

            // Previous frame texture (input) - needs COPY_DST since we copy to it from current
            self.working_texture_previous =
                Some(self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Working Texture Previous"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                }));

            // Delta texture (output from delta calculation, input for reconstruction)
            self.working_texture_delta =
                Some(self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Working Texture Delta"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Sint,
                    usage: wgpu::TextureUsages::STORAGE_BINDING
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_SRC
                        | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                }));

            // Output texture for reconstruction
            self.working_texture_output =
                Some(self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("Working Texture Output"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                }));

            // Calculate aligned buffer size for staging buffer
            let unpadded_bytes_per_row = width * 4; // 4 bytes per pixel for RGBA8Sint
            let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
            let buffer_size = (padded_bytes_per_row * height) as u64;

            self.staging_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Delta Staging Buffer"),
                size: buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }));

            self.current_dimensions = (width, height);
        }
    }

    fn calculate_aligned_bytes_per_row(width: u32) -> u32 {
        let unpadded_bytes_per_row = width * 4; // 4 bytes per pixel for RGBA
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        ((unpadded_bytes_per_row + align - 1) / align) * align
    }

    pub async fn compress_sequence(&mut self, images: &[RgbaImage]) -> Result<CompressedSequence> {
        if images.is_empty() {
            return Err(anyhow::anyhow!("No images to compress"));
        }

        let first_image = &images[0];
        let (width, height) = first_image.dimensions();

        log::info!(
            "Compressing sequence of {} frames ({}x{})",
            images.len(),
            width,
            height
        );

        self.ensure_working_textures(width, height);

        let mut deltas = Vec::with_capacity(images.len() - 1);

        // Upload first image as previous frame
        self.upload_image_to_texture(first_image, self.working_texture_previous.as_ref().unwrap())?;

        for (i, current_image) in images.iter().enumerate().skip(1) {
            log::debug!("Calculating delta for frame {}", i);

            // Upload current image
            self.upload_image_to_texture(
                current_image,
                self.working_texture_current.as_ref().unwrap(),
            )?;

            // Calculate delta
            let delta = self.calculate_delta().await?;
            deltas.push(delta);

            // Copy current to previous for next iteration
            self.copy_texture_to_texture(
                self.working_texture_current.as_ref().unwrap(),
                self.working_texture_previous.as_ref().unwrap(),
            )?;
        }

        log::info!(
            "Successfully compressed {} frames into {} deltas",
            images.len(),
            deltas.len()
        );

        Ok(CompressedSequence {
            base_frame: first_image.clone(),
            deltas,
            frame_count: images.len(),
        })
    }

    pub async fn reconstruct_frame(
        &mut self,
        base_frame: &RgbaImage,
        delta: &DeltaFrame,
    ) -> Result<RgbaImage> {
        let (width, height) = (delta.width, delta.height);
        self.ensure_working_textures(width, height);

        // Upload base frame
        self.upload_image_to_texture(base_frame, self.working_texture_current.as_ref().unwrap())?;

        // Upload delta data
        self.upload_delta_to_texture(delta)?;

        // Reconstruct frame
        self.reconstruct_frame_compute().await?;

        // Read back result
        self.read_reconstructed_frame(width, height).await
    }

    fn upload_image_to_texture(&self, image: &RgbaImage, texture: &wgpu::Texture) -> Result<()> {
        let (width, height) = image.dimensions();

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            image,
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

        Ok(())
    }

    fn upload_delta_to_texture(&self, delta: &DeltaFrame) -> Result<()> {
        let unpadded_bytes_per_row = 4 * delta.width;

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: self.working_texture_delta.as_ref().unwrap(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&delta.data),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(unpadded_bytes_per_row),
                rows_per_image: Some(delta.height),
            },
            wgpu::Extent3d {
                width: delta.width,
                height: delta.height,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }

    fn copy_texture_to_texture(&self, src: &wgpu::Texture, dst: &wgpu::Texture) -> Result<()> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Texture Copy Encoder"),
            });

        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: src,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: dst,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.current_dimensions.0,
                height: self.current_dimensions.1,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    async fn calculate_delta(&self) -> Result<DeltaFrame> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Delta Calculate Encoder"),
            });

        // Create bind group
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Delta Calculate Bind Group"),
            layout: &self.delta_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &self
                            .working_texture_current
                            .as_ref()
                            .unwrap()
                            .create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &self
                            .working_texture_previous
                            .as_ref()
                            .unwrap()
                            .create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &self
                            .working_texture_delta
                            .as_ref()
                            .unwrap()
                            .create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
            ],
        });

        // Dispatch compute shader
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Delta Calculate Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.delta_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);

            let (width, height) = self.current_dimensions;
            let workgroup_count_x = (width + 7) / 8;
            let workgroup_count_y = (height + 7) / 8;

            compute_pass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
        }

        // Copy result to staging buffer with proper alignment
        let (width, height) = self.current_dimensions;
        let padded_bytes_per_row = Self::calculate_aligned_bytes_per_row(width);

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: self.working_texture_delta.as_ref().unwrap(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: self.staging_buffer.as_ref().unwrap(),
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Map and read buffer
        let buffer_slice = self.staging_buffer.as_ref().unwrap().slice(..);
        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });

        let _ = self.device.poll(wgpu::MaintainBase::Wait);
        receiver.receive().await.unwrap()?;

        let data = buffer_slice.get_mapped_range();

        // Extract the actual image data, removing padding
        let mut delta_data = Vec::new();
        let unpadded_bytes_per_row = width * 4;

        for row in 0..height {
            let row_start = (row * padded_bytes_per_row) as usize;
            let row_end = row_start + unpadded_bytes_per_row as usize;
            let row_data = &data[row_start..row_end];
            delta_data.extend_from_slice(bytemuck::cast_slice::<u8, i8>(row_data));
        }

        drop(data);
        self.staging_buffer.as_ref().unwrap().unmap();

        Ok(DeltaFrame {
            data: delta_data,
            width,
            height,
        })
    }

    async fn reconstruct_frame_compute(&self) -> Result<()> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame Reconstruct Encoder"),
            });

        // Create bind group
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Frame Reconstruct Bind Group"),
            layout: &self.reconstruct_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &self
                            .working_texture_current
                            .as_ref()
                            .unwrap()
                            .create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &self
                            .working_texture_delta
                            .as_ref()
                            .unwrap()
                            .create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &self
                            .working_texture_output
                            .as_ref()
                            .unwrap()
                            .create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
            ],
        });

        // Dispatch compute shader
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Frame Reconstruct Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.reconstruct_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);

            let (width, height) = self.current_dimensions;
            let workgroup_count_x = (width + 7) / 8;
            let workgroup_count_y = (height + 7) / 8;

            compute_pass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    async fn read_reconstructed_frame(&self, width: u32, height: u32) -> Result<RgbaImage> {
        let padded_bytes_per_row = Self::calculate_aligned_bytes_per_row(width);
        let buffer_size = (padded_bytes_per_row * height) as u64;

        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Frame Output Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Read Frame Encoder"),
            });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: self.working_texture_output.as_ref().unwrap(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &output_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Map and read buffer
        let buffer_slice = output_buffer.slice(..);
        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });

        let _ = self.device.poll(wgpu::MaintainBase::Wait);
        receiver.receive().await.unwrap()?;

        let data = buffer_slice.get_mapped_range();

        // Extract the actual image data, removing padding if necessary
        let mut image_data = Vec::new();
        let unpadded_bytes_per_row = width * 4;

        if padded_bytes_per_row == unpadded_bytes_per_row {
            // No padding, can copy directly
            image_data = data.to_vec();
        } else {
            // Remove padding from each row
            for row in 0..height {
                let row_start = (row * padded_bytes_per_row) as usize;
                let row_end = row_start + unpadded_bytes_per_row as usize;
                image_data.extend_from_slice(&data[row_start..row_end]);
            }
        }

        drop(data);

        RgbaImage::from_raw(width, height, image_data)
            .ok_or_else(|| anyhow::anyhow!("Failed to create image from reconstructed data"))
    }
}

impl CompressedSequence {
    pub fn memory_usage(&self) -> usize {
        let base_size = self.base_frame.as_raw().len();
        let deltas_size: usize = self.deltas.iter().map(|d| d.data.len()).sum();
        base_size + deltas_size
    }

    pub fn compression_ratio(&self, original_size: usize) -> f32 {
        let compressed_size = self.memory_usage();
        original_size as f32 / compressed_size as f32
    }
}
