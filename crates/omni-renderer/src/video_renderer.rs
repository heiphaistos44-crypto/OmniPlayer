use anyhow::{Context as _, Result};
use wgpu::{util::DeviceExt, *};

use crate::frame_upload::YuvTextures;
use omni_core::decoder::DecodedVideoFrame;

const SHADER_SRC: &str = include_str!("../../../assets/shaders/yuv_to_rgb.wgsl");

/// Renderer wgpu : upload YUV → rendu RGB via shader WGSL.
/// S'intègre dans egui via `egui_wgpu::CallbackFn`.
pub struct VideoRenderer {
    pipeline:        RenderPipeline,
    sampler:         Sampler,
    bind_group_layout: BindGroupLayout,
    bind_group:      Option<BindGroup>,
    yuv_textures:    Option<YuvTextures>,
    uniform_buf:     Buffer,
    uniform_bg:      BindGroup,
    uniform_bgl:     BindGroupLayout,
}

/// Uniforms envoyés au shader (matrice couleur + offset).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ColorUniforms {
    /// Matrice 4×4 (BT.601 / BT.709 / BT.2020) — row-major
    matrix: [[f32; 4]; 4],
    offset: [f32; 4],
}

impl ColorUniforms {
    fn bt709() -> Self {
        // BT.709 (HDTV/1080p+)
        Self {
            matrix: [
                [1.0,     0.0,     1.5748, 0.0],
                [1.0,    -0.1873, -0.4681, 0.0],
                [1.0,     1.8556,  0.0,    0.0],
                [0.0,     0.0,     0.0,    1.0],
            ],
            offset: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

impl VideoRenderer {
    pub fn new(device: &Device, surface_format: TextureFormat) -> Result<Self> {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label:  Some("yuv_to_rgb"),
            source: ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let sampler = device.create_sampler(&SamplerDescriptor {
            label:        Some("yuv_sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            mag_filter:   FilterMode::Linear,
            min_filter:   FilterMode::Linear,
            ..Default::default()
        });

        // BGL pour les 3 textures YUV + sampler
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label:   Some("yuv_bgl"),
            entries: &[
                texture_entry(0), texture_entry(1), texture_entry(2),
                BindGroupLayoutEntry {
                    binding:    3,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // BGL uniforms couleur
        let uniform_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label:   Some("color_uniform_bgl"),
            entries: &[BindGroupLayoutEntry {
                binding:    0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty:                 BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size:   None,
                },
                count: None,
            }],
        });

        let uniforms    = ColorUniforms::bt709();
        let uniform_buf = device.create_buffer_init(&util::BufferInitDescriptor {
            label:    Some("color_uniform_buf"),
            contents: bytemuck::bytes_of(&uniforms),
            usage:    BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let uniform_bg = device.create_bind_group(&BindGroupDescriptor {
            label:   Some("color_uniform_bg"),
            layout:  &uniform_bgl,
            entries: &[BindGroupEntry {
                binding:  0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label:                Some("video_pipeline_layout"),
            bind_group_layouts:   &[&bind_group_layout, &uniform_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label:       Some("video_pipeline"),
            layout:      Some(&pipeline_layout),
            vertex:      VertexState {
                module:      &shader,
                entry_point: Some("vs_main"),
                buffers:     &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module:      &shader,
                entry_point: Some("fs_main"),
                targets:     &[Some(ColorTargetState {
                    format:     surface_format,
                    blend:      None,
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive:    PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample:   MultisampleState::default(),
            multiview:     None,
            cache:         None,
        });

        Ok(Self {
            pipeline,
            sampler,
            bind_group_layout,
            bind_group: None,
            yuv_textures: None,
            uniform_buf,
            uniform_bg,
            uniform_bgl,
        })
    }

    /// Met à jour les textures avec un nouveau frame.
    pub fn upload_frame(&mut self, device: &Device, queue: &Queue, frame: &DecodedVideoFrame) {
        let textures = YuvTextures::ensure(
            self.yuv_textures.take(),
            device,
            frame.width,
            frame.height,
        );
        textures.upload(queue, frame);

        let yv = textures.y.create_view(&Default::default());
        let uv = textures.u.create_view(&Default::default());
        let vv = textures.v.create_view(&Default::default());

        self.bind_group = Some(device.create_bind_group(&BindGroupDescriptor {
            label:   Some("yuv_bg"),
            layout:  &self.bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: BindingResource::TextureView(&yv) },
                BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&uv) },
                BindGroupEntry { binding: 2, resource: BindingResource::TextureView(&vv) },
                BindGroupEntry { binding: 3, resource: BindingResource::Sampler(&self.sampler) },
            ],
        }));

        self.yuv_textures = Some(textures);
    }

    /// Encode le pass de rendu vidéo dans un RenderPass existant.
    pub fn render(&self, rp: &mut RenderPass<'static>) {
        if let Some(bg) = &self.bind_group {
            rp.set_pipeline(&self.pipeline);
            rp.set_bind_group(0, bg, &[]);
            rp.set_bind_group(1, &self.uniform_bg, &[]);
            rp.draw(0..4, 0..1);
        }
    }
}

fn texture_entry(binding: u32) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type:    TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled:   false,
        },
        count: None,
    }
}
