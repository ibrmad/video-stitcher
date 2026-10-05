//! Draw a texture fitted and centred into a render target, the rest black:
//! the lens preview's one camera inside the preview's frame. Reco's
//! `LensPreviewRenderer` draws at the input's size; this brings it to the
//! frame's.

use reco_core::gpu::GpuContext;
use reco_core::wgpu;

/// Where a `source`-sized picture sits fitted and centred in a
/// `target`-sized frame: (x, y, width, height) in the frame's pixels.
pub fn fitted(source: (u32, u32), target: (u32, u32)) -> (f32, f32, f32, f32) {
    let (sw, sh) = (source.0 as f32, source.1 as f32);
    let (tw, th) = (target.0 as f32, target.1 as f32);
    if sw <= 0.0 || sh <= 0.0 || tw <= 0.0 || th <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let scale = (tw / sw).min(th / sh);
    let (w, h) = (sw * scale, sh * scale);
    ((tw - w) / 2.0, (th - h) / 2.0, w, h)
}

const SHADER: &str = r"
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
    // One triangle over the whole viewport.
    let corner = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: Out;
    out.position = vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(corner.x, 1.0 - corner.y);
    return out;
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;

@fragment
fn fs(in: Out) -> @location(0) vec4<f32> {
    return textureSample(picture, linear_sampler, in.uv);
}
";

/// The pass that draws a picture fitted into a target.
pub struct Fit {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Fit {
    /// A pass writing `format` targets.
    pub fn new(gpu: &GpuContext, format: wgpu::TextureFormat) -> Self {
        let device = gpu.device();
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fit"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fit"),
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
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fit"),
            bind_group_layouts: &[&layout],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fit"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fit"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            pipeline,
            layout,
            sampler,
        }
    }

    /// Draw `picture` fitted and centred into `target` (`size` pixels),
    /// black around it; submits.
    pub fn draw(
        &self,
        gpu: &GpuContext,
        picture: &wgpu::Texture,
        target: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        let device = gpu.device();
        let view = picture.create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fit"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("fit") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let (x, y, w, h) = fitted((picture.width(), picture.height()), size);
            if w >= 1.0 && h >= 1.0 {
                pass.set_viewport(x, y, w, h, 0.0, 1.0);
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        gpu.queue().submit([encoder.finish()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_fits_inside_and_centres() {
        // 4:3 into 16:9: bars at the sides.
        assert_eq!(fitted((1280, 960), (320, 180)), (40.0, 0.0, 240.0, 180.0));
        // 16:9 into 4:3: bars above and below.
        assert_eq!(fitted((1920, 1080), (400, 300)), (0.0, 37.5, 400.0, 225.0));
        // The same shape fills it.
        assert_eq!(fitted((640, 360), (320, 180)), (0.0, 0.0, 320.0, 180.0));
        assert_eq!(
            fitted((0, 0), (320, 180)),
            (0.0, 0.0, 0.0, 0.0),
            "no picture"
        );
    }
}
