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
}

impl DynamicBackdropProgram {
    pub fn new(
        palette: AlbumPalette,
        time_s: f32,
        transparent_acrylic_tint: Option<f32>,
    ) -> Self {
        Self {
            palette,
            time_s,
            transparent_acrylic_tint,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackdropPrimitive {
    uniforms: BackdropUniforms,
}

pub struct BackdropPipeline {
    render_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl shader::Pipeline for BackdropPipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
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

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lume_music.backdrop.bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lume_music.backdrop.bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
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
            bind_group,
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
        BackdropPrimitive {
            uniforms: BackdropUniforms {
                colors: self.palette.colors,
                params: [self.time_s, self.palette.scrim_alpha, acrylic_alpha, 0.0],
            },
        }
    }
}
