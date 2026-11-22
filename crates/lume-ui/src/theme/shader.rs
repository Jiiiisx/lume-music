use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use iced::mouse;
use iced::wgpu;
use iced::widget::shader::{self, Viewport};
use iced::Rectangle;

use crate::theme::palette::AlbumPalette;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct BackdropUniforms {
    pub colors: [[f32; 4]; 4],
    pub params: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct DynamicBackdropProgram {
    pub palette: AlbumPalette,
    pub time_s: f32,
    pub transparent_acrylic_tint: Option<f32>,
    pub art_rgba: Option<Arc<Vec<u8>>>,
    pub art_version: u64,
}

impl DynamicBackdropProgram {
    pub fn new(
        palette: AlbumPalette,
        time_s: f32,
        transparent_acrylic_tint: Option<f32>,
        art_rgba: Option<Arc<Vec<u8>>>,
        art_version: u64,
    ) -> Self {
        Self {
            palette,
            time_s,
            transparent_acrylic_tint,
            art_rgba,
            art_version,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackdropPrimitive {
    uniforms: BackdropUniforms,
    art_rgba: Option<Arc<Vec<u8>>>,
    art_version: u64,
}

pub struct BackdropPipeline {
    render_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    art_texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    uploaded_version: u64,
}

impl shader::Pipeline for BackdropPipeline {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lume_music.backdrop.shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("background.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lume_music.backdrop.uniforms"),
            size: std::mem::size_of::<BackdropUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let art_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("lume_music.backdrop.art_tex"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Seed neutral pixels so initial bind group is well-defined before any album art uploads.
        let init_pixels = vec![32u8; 64 * 64 * 4];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &art_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &init_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(64 * 4),
                rows_per_image: Some(64),
            },
            wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
        );

        let art_view = art_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let art_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("lume_music.backdrop.art_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lume_music.backdrop.bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lume_music.backdrop.bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&art_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&art_sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lume_music.backdrop.pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lume_music.backdrop.pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            render_pipeline,
            uniform_buffer,
            art_texture,
            bind_group,
            uploaded_version: 0,
        }
    }
}

impl shader::Primitive for BackdropPrimitive {
    type Pipeline = BackdropPipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &Viewport,
    ) {
        if self.art_version != pipeline.uploaded_version {
            if let Some(bytes) = &self.art_rgba {
                if bytes.len() == 64 * 64 * 4 {
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &pipeline.art_texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        bytes,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(64 * 4),
                            rows_per_image: Some(64),
                        },
                        wgpu::Extent3d {
                            width: 64,
                            height: 64,
                            depth_or_array_layers: 1,
                        },
                    );
                }
            }
            pipeline.uploaded_version = self.art_version;
        }

        queue.write_buffer(
            &pipeline.uniform_buffer,
            0,
            bytemuck::bytes_of(&self.uniforms),
        );
    }

    fn draw(&self, pipeline: &Self::Pipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        render_pass.set_pipeline(&pipeline.render_pipeline);
        render_pass.set_bind_group(0, &pipeline.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
        true
    }
}

impl<Message> shader::Program<Message> for DynamicBackdropProgram {
    type State = ();
    type Primitive = BackdropPrimitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        _bounds: Rectangle,
    ) -> Self::Primitive {
        let acrylic_alpha = self.transparent_acrylic_tint.unwrap_or(1.0);
        let has_art = self
            .art_rgba
            .as_ref()
            .map(|b| b.len() == 64 * 64 * 4)
            .unwrap_or(false);
        BackdropPrimitive {
            uniforms: BackdropUniforms {
                colors: self.palette.colors,
                params: [
                    self.time_s,
                    self.palette.scrim_alpha,
                    acrylic_alpha,
                    if has_art { 1.0 } else { 0.0 },
                ],
            },
            art_rgba: self.art_rgba.clone(),
            art_version: self.art_version,
        }
    }
}
