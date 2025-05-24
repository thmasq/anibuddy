use std::{collections::HashMap, num::NonZeroU64, sync::Arc};

use bytemuck::{Pod, Zeroable, cast_slice};
use wgpu::{
    self, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType, Buffer,
    BufferBinding, BufferBindingType, BufferDescriptor, BufferUsages, ComputePass, ComputePipeline,
    ComputePipelineDescriptor, Device, PipelineCompilationOptions, PipelineLayoutDescriptor, Queue,
    ShaderModule, ShaderStages, TextureSampleType, TextureView, TextureViewDimension, include_wgsl,
};

use crate::bc7::{BC7Settings, CompressionVariant};

#[derive(Copy, Clone, Zeroable, Pod)]
#[repr(C)]
struct Uniforms {
    /// The width of the image data.
    width: u32,
    /// The height of the image data.
    height: u32,
    /// Start row of the texture data we want to convert.
    texture_y_offset: u32,
    /// Start of the blocks data in u32 elements.
    blocks_offset: u32,
}

struct Task {
    variant: CompressionVariant,
    width: u32,
    height: u32,
    uniform_offset: u32,
    setting_offset: u32,
    texture_y_offset: u32,
    buffer_offset: u32,
    texture_view: TextureView,
    buffer: Buffer,
}

/// Compresses texture data with a block compression algorithm using WGPU compute shader.
pub struct GpuBlockCompressor {
    scratch_buffer: Vec<u8>,
    task: Vec<Task>,
    uniforms_buffer: Buffer,
    bc7_settings_buffer: Buffer,
    bind_group_layouts: HashMap<CompressionVariant, BindGroupLayout>,
    pipelines: HashMap<CompressionVariant, ComputePipeline>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    uniforms_aligned_size: usize,
    bc7_aligned_size: usize,
}

impl GpuBlockCompressor {
    /// Creates a new block compressor instance.
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        let limits = device.limits();

        let alignment = limits.min_uniform_buffer_offset_alignment as usize;
        let size = size_of::<Uniforms>();
        let uniforms_aligned_size = size.div_ceil(alignment) * alignment;

        let bc7_aligned_size = {
            let alignment = limits.min_storage_buffer_offset_alignment as usize;
            let size = size_of::<BC7Settings>();
            size.div_ceil(alignment) * alignment
        };

        let shader_module_bc7 = device.create_shader_module(include_wgsl!("shader/bc7.wgsl"));

        let uniforms_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("uniforms"),
            size: (uniforms_aligned_size * 16) as _,
            usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        let bc7_settings_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("bc7 settings"),
            size: (bc7_aligned_size * 16) as _,
            usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let mut bind_group_layouts = HashMap::new();
        let mut pipelines = HashMap::new();

        Self::create_pipeline(
            &device,
            &shader_module_bc7,
            &mut bind_group_layouts,
            &mut pipelines,
            CompressionVariant::BC7(BC7Settings::alpha_basic()),
        );

        Self {
            scratch_buffer: Vec::default(),
            task: Vec::default(),
            uniforms_buffer,
            bc7_settings_buffer,
            bind_group_layouts,
            pipelines,
            device,
            queue,
            uniforms_aligned_size,
            bc7_aligned_size,
        }
    }

    #[allow(unused_mut)]
    fn create_pipeline(
        device: &Device,
        shader_module: &ShaderModule,
        bind_group_layouts: &mut HashMap<CompressionVariant, BindGroupLayout>,
        pipelines: &mut HashMap<CompressionVariant, ComputePipeline>,
        variant: CompressionVariant,
    ) {
        let mut layout_entries = vec![
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: None,
                },
                count: None,
            },
        ];

        match variant {
            CompressionVariant::BC7(..) => {
                layout_entries.push(BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(size_of::<BC7Settings>() as _),
                    },
                    count: None,
                });
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }

        let name = variant.name();

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some(&format!("{name} bind group layout")),
            entries: &layout_entries,
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some(&format!("{name} block compression pipeline layout")),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some(&format!("{name} block compression pipeline")),
            layout: Some(&pipeline_layout),
            module: shader_module,
            entry_point: Some(variant.entry_point()),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });

        bind_group_layouts.insert(variant, bind_group_layout);
        pipelines.insert(variant, pipeline);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_compression_task(
        &mut self,
        variant: CompressionVariant,
        texture_view: &TextureView,
        width: u32,
        height: u32,
        buffer: &Buffer,
        texture_y_offset: Option<u32>,
        blocks_offset: Option<u32>,
    ) {
        assert_eq!(height % 4, 0);
        assert_eq!(width % 4, 0);

        if let Some(texture_y_offset) = texture_y_offset {
            assert_eq!(texture_y_offset % 4, 0);
        }

        assert!(
            buffer.usage().contains(BufferUsages::STORAGE),
            "buffer needs to be a storage buffer"
        );

        let required_size = variant.clone().blocks_byte_size(width, height);
        let total_size = blocks_offset.unwrap_or(0) as usize + required_size;

        assert!(
            buffer.size() as usize >= total_size,
            "buffer size ({}) is too small to hold compressed blocks at offset {}. Required size: {}",
            buffer.size(),
            blocks_offset.unwrap_or(0),
            total_size
        );

        self.task.push(Task {
            variant,
            width,
            height,
            uniform_offset: 0,
            setting_offset: 0,
            texture_y_offset: texture_y_offset.unwrap_or(0),
            buffer_offset: blocks_offset.unwrap_or(0),
            texture_view: texture_view.clone(),
            buffer: buffer.clone(),
        });
    }

    fn update_buffer_sizes(&mut self) {
        let total_uniforms_size = self.uniforms_aligned_size * self.task.len();
        if total_uniforms_size > self.uniforms_buffer.size() as usize {
            self.uniforms_buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some("uniforms buffer"),
                size: total_uniforms_size as u64,
                usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
                mapped_at_creation: false,
            });
        }

        let bc7_setting_count = self
            .task
            .iter()
            .filter(|task| matches!(task.variant, CompressionVariant::BC7(..)))
            .count();

        let total_bc7_size = self.bc7_aligned_size * bc7_setting_count;
        if total_bc7_size > self.bc7_settings_buffer.size() as usize {
            self.bc7_settings_buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some("bc7 settings buffer"),
                size: total_bc7_size as u64,
                usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
                mapped_at_creation: false,
            });
        }
    }

    fn upload(&mut self) {
        self.scratch_buffer.clear();
        for (index, task) in self.task.iter_mut().enumerate() {
            let offset = index * self.uniforms_aligned_size;
            task.uniform_offset = offset as u32;

            let uniforms = Uniforms {
                width: task.width,
                height: task.height,
                texture_y_offset: task.texture_y_offset,
                blocks_offset: task.buffer_offset / 4,
            };

            self.scratch_buffer
                .resize(offset + self.uniforms_aligned_size, 0);
            self.scratch_buffer[offset..offset + size_of::<Uniforms>()]
                .copy_from_slice(cast_slice(&[uniforms]));
        }
        if !self.scratch_buffer.is_empty() {
            if let Some(mut data) = self.queue.write_buffer_with(
                &self.uniforms_buffer,
                0,
                NonZeroU64::new(self.scratch_buffer.len() as u64).unwrap(),
            ) {
                data.copy_from_slice(&self.scratch_buffer);
            }
        }

        self.scratch_buffer.clear();
        for (index, (settings, task)) in self
            .task
            .iter_mut()
            .filter_map(|task| {
                #[allow(irrefutable_let_patterns)]
                if let CompressionVariant::BC7(settings) = task.variant {
                    Some((settings, task))
                } else {
                    None
                }
            })
            .enumerate()
        {
            let offset = index * self.bc7_aligned_size;
            task.setting_offset = offset as u32;
            self.scratch_buffer
                .resize(offset + self.bc7_aligned_size, 0);
            self.scratch_buffer[offset..offset + size_of::<BC7Settings>()]
                .copy_from_slice(cast_slice(&[settings]));
        }
        if !self.scratch_buffer.is_empty() {
            if let Some(mut data) = self.queue.write_buffer_with(
                &self.bc7_settings_buffer,
                0,
                NonZeroU64::new(self.scratch_buffer.len() as u64).unwrap(),
            ) {
                data.copy_from_slice(&self.scratch_buffer);
            }
        }
    }

    pub fn compress(&mut self, pass: &mut ComputePass) {
        self.update_buffer_sizes();
        self.upload();

        let mut bind_groups: Vec<BindGroup> = self
            .task
            .iter()
            .map(|task| self.create_bind_group(task))
            .collect();

        for (task, bind_group) in self.task.drain(..).zip(bind_groups.drain(..)) {
            let pipeline = self
                .pipelines
                .get(&task.variant)
                .expect("can't find pipeline for variant");

            pass.set_pipeline(pipeline);

            match task.variant {
                CompressionVariant::BC7(..) => {
                    pass.set_bind_group(
                        0,
                        &bind_group,
                        &[task.uniform_offset, task.setting_offset],
                    );
                }
                #[allow(irrefutable_let_patterns)]
                #[allow(unreachable_patterns)]
                _ => {
                    pass.set_bind_group(0, &bind_group, &[task.uniform_offset]);
                }
            }

            let block_width = (task.width + 3) / 4;
            let block_height = (task.height + 3) / 4;

            let workgroup_width = (block_width + 7) / 8;
            let workgroup_height = (block_height + 7) / 8;

            pass.dispatch_workgroups(workgroup_width, workgroup_height, 1);
        }
    }

    fn create_bind_group(&self, task: &Task) -> BindGroup {
        let bind_group_layout = self
            .bind_group_layouts
            .get(&task.variant)
            .expect("Can't find bind group layout for variant");

        match task.variant {
            CompressionVariant::BC7(..) => self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("bind group"),
                layout: bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&task.texture_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: task.buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: &self.uniforms_buffer,
                            offset: 0,
                            size: Some(NonZeroU64::new(self.uniforms_aligned_size as u64).unwrap()),
                        }),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: &self.bc7_settings_buffer,
                            offset: 0,
                            size: Some(NonZeroU64::new(self.bc7_aligned_size as u64).unwrap()),
                        }),
                    },
                ],
            }),
        }
    }
}
