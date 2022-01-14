use nannou::prelude::*;

use crate::geometry::*;

pub struct CustomRenderer {
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    pub output_texture: wgpu::Texture,
    pub texture_reshaper: wgpu::TextureReshaper,
}

/// A render pipeline generator for a fragment shader with optional textures, sampler, and uniform buffer
impl CustomRenderer {
    pub fn new<T>(
        device: &wgpu::Device,
        vs_mod: &wgpu::ShaderModule,
        fs_mod: &wgpu::ShaderModule,
        uniform_textures: Option<&Vec<&wgpu::Texture>>,
        sampler: Option<&wgpu::Sampler>,
        uniform_buffer: Option<&wgpu::Buffer>,
        width: u32,
        height: u32,
        sample_count: u32,
    ) -> Self
    where
        T: Copy,
    {
        println!("creating bind group");

        let mut bind_group_layout_builder = wgpu::BindGroupLayoutBuilder::new();
        let mut bind_group_builder = wgpu::BindGroupBuilder::new();

        let texture_views = match uniform_textures {
            Some(textures) => Some(
                textures
                    .iter()
                    .map(|t| t.view().build())
                    .collect::<Vec<wgpu::TextureView>>(),
            ),
            None => None,
        };

        if let Some(textures) = uniform_textures {
            for t in textures.iter() {
                bind_group_layout_builder = bind_group_layout_builder.sampled_texture(
                    wgpu::ShaderStage::FRAGMENT,
                    true,
                    wgpu::TextureViewDimension::D2,
                    t.component_type(),
                )
            }

            if let Some(views) = texture_views.as_ref() {
                for v in views {
                    bind_group_builder = bind_group_builder.texture_view(v);
                }
            }
        }

        if let Some(ref s) = sampler {
            bind_group_layout_builder =
                bind_group_layout_builder.sampler(wgpu::ShaderStage::FRAGMENT);

            bind_group_builder = bind_group_builder.sampler(s);
        }

        if let Some(ref buffer) = uniform_buffer {
            bind_group_layout_builder =
                bind_group_layout_builder.uniform_buffer(wgpu::ShaderStage::FRAGMENT, false);

            bind_group_builder = bind_group_builder.buffer::<T>(buffer, 0..1);
        }

        let bind_group_layout = bind_group_layout_builder.build(device);
        let bind_group = bind_group_builder.build(device, &bind_group_layout);

        println!("creating pipeline layout");
        let pipeline_layout = create_pipeline_layout(device, &bind_group_layout);

        println!("creating render pipeline");
        let render_pipeline = create_render_pipeline(device, &pipeline_layout, &vs_mod, &fs_mod, 1);

        let output_texture = create_app_texture(&device, width, height, 1);

        let texture_reshaper = create_texture_reshaper(&device, &output_texture, 1, sample_count);

        Self {
            bind_group,
            render_pipeline,
            output_texture,
            texture_reshaper,
        }
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, vertex_buffer: &wgpu::Buffer) {
        let texture_view = self.output_texture.view().build();
        let mut render_pass = wgpu::RenderPassBuilder::new()
            .color_attachment(&texture_view, |color| color)
            .begin(encoder);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_vertex_buffer(0, vertex_buffer, 0, 0);
        let vertex_range = 0..VERTICES.len() as u32;
        let instance_range = 0..1;
        render_pass.draw(vertex_range, instance_range);
    }
}

pub fn create_app_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    msaa_samples: u32,
) -> wgpu::Texture {
    wgpu::TextureBuilder::new()
        .size([width, height])
        .usage(
            wgpu::TextureUsage::OUTPUT_ATTACHMENT
                | wgpu::TextureUsage::SAMPLED
                | wgpu::TextureUsage::COPY_SRC
                | wgpu::TextureUsage::COPY_DST,
        )
        .sample_count(msaa_samples)
        .format(Frame::TEXTURE_FORMAT)
        .build(device)
}

fn create_texture_reshaper(
    device: &wgpu::Device,
    texture: &wgpu::Texture,
    src_sample_count: u32,
    dst_sample_count: u32,
) -> wgpu::TextureReshaper {
    let texture_view = texture.view().build();
    let texture_component_type = texture.component_type();
    let dst_format = Frame::TEXTURE_FORMAT;
    wgpu::TextureReshaper::new(
        device,
        &texture_view,
        src_sample_count,
        texture_component_type,
        dst_sample_count,
        dst_format,
    )
}

fn create_pipeline_layout(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::PipelineLayout {
    let desc = wgpu::PipelineLayoutDescriptor {
        bind_group_layouts: &[&bind_group_layout],
    };
    device.create_pipeline_layout(&desc)
}

pub fn create_render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    vs_mod: &wgpu::ShaderModule,
    fs_mod: &wgpu::ShaderModule,
    sample_count: u32,
) -> wgpu::RenderPipeline {
    wgpu::RenderPipelineBuilder::from_layout(layout, vs_mod)
        .fragment_shader(fs_mod)
        .color_format(Frame::TEXTURE_FORMAT)
        .add_vertex_buffer::<Vertex>(&wgpu::vertex_attr_array![0 => Float2])
        .sample_count(sample_count)
        .primitive_topology(wgpu::PrimitiveTopology::TriangleStrip)
        .build(device)
}
