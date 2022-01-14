use glsl_layout::float;
use glsl_layout::*;
use nannou::image::GenericImageView;
use nannou::image::{self, DynamicImage};
use nannou::prelude::*;
use std::fs;

struct Model {
    width: u32,
    height: u32,
    uniform_texture: wgpu::Texture,
    texture_capturer: wgpu::TextureCapturer,
    texture_reshaper: wgpu::TextureReshaper,
    field_generator: CustomRenderer,
    sorter: CustomRenderer,
    vertex_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
}

// The vertex type that we will use to represent a point on our triangle.
#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 2],
}

// The vertices that make up the rectangle to which the image will be drawn.
const VERTICES: [Vertex; 4] = [
    Vertex {
        position: [-1.0, 1.0],
    },
    Vertex {
        position: [-1.0, -1.0],
    },
    Vertex {
        position: [1.0, 1.0],
    },
    Vertex {
        position: [1.0, -1.0],
    },
];

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Uniform)]
pub struct Uniforms {
    iteration: uint,
    width: float,
    height: float,
}

fn main() {
    nannou::app(model).update(update).run();
}

fn model(app: &App) -> Model {
    // Load the image.
    let image_path = app.assets_path().unwrap().join("vqgan_graffiti.png");
    let image = image::open(image_path).unwrap();
    let (width, height) = image.dimensions();

    let window_id = app
        .new_window()
        .size(width, height)
        .view(view)
        .build()
        .unwrap();
    let window = app.window(window_id).unwrap();
    let device = window.swap_chain_device();
    let sample_count = window.msaa_samples();

    // Create the compute shader module.
    println!("loading shaders");
    let vs_mod = compile_shader(app, &device, "shader.vert", shaderc::ShaderKind::Vertex);
    let field_fs_mod = compile_shader(app, &device, "field.frag", shaderc::ShaderKind::Fragment);
    let sort_fs_mod = compile_shader(app, &device, "sort.frag", shaderc::ShaderKind::Fragment);

    let uniform_texture = create_app_texture(&device, width, height, 1);

    // Create the sampler for sampling from the source texture.
    let sampler = wgpu::SamplerBuilder::new().build(device);

    // create uniform buffer
    let uniforms = create_uniforms(width, height, 0);
    println!("uniforms: {:?}", uniforms);
    let uniforms_bytes = uniforms_as_bytes(&uniforms);
    let uniform_buffer = device.create_buffer_with_data(
        uniforms_bytes,
        wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
    );

    let field_generator = CustomRenderer::new(
        &device,
        &vs_mod,
        &field_fs_mod,
        None,
        None,
        Some(&uniform_buffer),
        width,
        height,
    );

    let sorter_uniform_textures = vec![&uniform_texture, &field_generator.output_texture];

    let sorter = CustomRenderer::new(
        &device,
        &vs_mod,
        &sort_fs_mod,
        Some(&sorter_uniform_textures),
        Some(&sampler),
        Some(&uniform_buffer),
        width,
        height,
    );

    // create our custom texture for rendering
    println!("creating app texure and reshaper");
    let texture_reshaper =
        create_texture_reshaper(&device, &sorter.output_texture, 1, sample_count);

    println!("creating vertex buffer");
    let vertices_bytes = vertices_as_bytes(&VERTICES[..]);
    let vertex_buffer = device.create_buffer_with_data(vertices_bytes, wgpu::BufferUsage::VERTEX);

    // Create the texture capturer.
    println!("creating texture capturer");
    let texture_capturer = wgpu::TextureCapturer::default();

    copy_image_to_texture(
        app,
        &window,
        &device,
        &image,
        &uniform_texture,
        1,
        &vertex_buffer,
    );

    std::fs::create_dir_all(&capture_directory(app)).unwrap();

    Model {
        width,
        height,
        uniform_texture,
        texture_capturer,
        texture_reshaper,
        field_generator,
        sorter,
        vertex_buffer,
        uniform_buffer,
    }
}

fn update(app: &App, model: &mut Model, _update: Update) {
    let window = app.main_window();
    let device = window.swap_chain_device();

    // An update for the uniform buffer with the current time.
    let elapsed_frames = app.main_window().elapsed_frames();
    let uniforms = create_uniforms(model.width, model.height, elapsed_frames as u32 % 2);
    let uniforms_bytes = uniforms_as_bytes(&uniforms);
    let uniforms_size = uniforms_bytes.len();
    let new_uniform_buffer =
        device.create_buffer_with_data(uniforms_bytes, wgpu::BufferUsage::COPY_SRC);

    // The encoder we'll use to encode the render pass
    let desc = wgpu::CommandEncoderDescriptor {
        label: Some("encoder"),
    };
    let mut encoder = device.create_command_encoder(&desc);

    encoder.copy_buffer_to_buffer(
        &new_uniform_buffer,
        0,
        &model.uniform_buffer,
        0,
        uniforms_size as u64,
    );

    // Take a snapshot of the texture. The capturer will do the following:
    //
    // 1. Resolve the texture to a non-multisampled texture if necessary.
    // 2. Convert the format to non-linear 8-bit sRGBA ready for image storage.
    // 3. Copy the result to a buffer ready to be mapped for reading.
    let snapshot = model
        .texture_capturer
        .capture(device, &mut encoder, &model.uniform_texture);

    model
        .field_generator
        .render(&mut encoder, &model.vertex_buffer);

    model.sorter.render(&mut encoder, &model.vertex_buffer);

    // copy app texture to uniform texture
    copy_texture(
        &mut encoder,
        &model.sorter.output_texture,
        &model.uniform_texture,
    );

    // submit encoded command buffer
    window.swap_chain_queue().submit(&[encoder.finish()]);

    // Submit a function for writing our snapshot to a PNG.
    //
    // NOTE: It is essential that the commands for capturing the snapshot are `submit`ted before we
    // attempt to read the snapshot - otherwise we will read a blank texture!
    let elapsed_frames = app.main_window().elapsed_frames();
    let path = capture_directory(app)
        .join(elapsed_frames.to_string())
        .with_extension("png");
    snapshot
        .read(move |result| {
            let image = result.expect("failed to map texture memory");
            image
                .save(&path)
                .expect("failed to save texture to png image");
        })
        .unwrap();

    // std::thread::sleep(std::time::Duration::from_secs(1));
}

fn view(_app: &App, model: &Model, frame: Frame) {
    // Sample the texture and write it to the frame.
    let mut encoder = frame.command_encoder();
    model
        .texture_reshaper
        .encode_render_pass(frame.texture_view(), &mut *encoder);
}

fn create_uniforms(width: u32, height: u32, iteration: u32) -> Uniforms {
    Uniforms {
        width: width as f32,
        height: height as f32,
        iteration,
    }
}

fn uniforms_as_bytes(uniforms: &Uniforms) -> &[u8] {
    unsafe { wgpu::bytes::from(uniforms) }
}

struct CustomRenderer {
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    pub output_texture: wgpu::Texture,
}

/// A render pipeline generator for a fragment shader with optional textures, sampler, and uniform buffer
impl CustomRenderer {
    pub fn new(
        device: &wgpu::Device,
        vs_mod: &wgpu::ShaderModule,
        fs_mod: &wgpu::ShaderModule,
        uniform_textures: Option<&Vec<&wgpu::Texture>>,
        sampler: Option<&wgpu::Sampler>,
        uniform_buffer: Option<&wgpu::Buffer>,
        width: u32,
        height: u32,
    ) -> Self {
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

            bind_group_builder = bind_group_builder.buffer::<Uniforms>(buffer, 0..1);
        }

        let bind_group_layout = bind_group_layout_builder.build(device);
        let bind_group = bind_group_builder.build(device, &bind_group_layout);

        println!("creating pipeline layout");
        let pipeline_layout = create_pipeline_layout(device, &bind_group_layout);

        println!("creating render pipeline");
        let render_pipeline = create_render_pipeline(device, &pipeline_layout, &vs_mod, &fs_mod, 1);

        let output_texture = create_app_texture(&device, width, height, 1);

        Self {
            bind_group,
            render_pipeline,
            output_texture,
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

fn create_app_texture(
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

fn create_render_pipeline(
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

/// See the `nannou::wgpu::bytes` documentation for why this is necessary.
fn vertices_as_bytes(data: &[Vertex]) -> &[u8] {
    unsafe { wgpu::bytes::from_slice(data) }
}

/// Copies the contents of a texture from one to another
pub fn copy_texture(encoder: &mut wgpu::CommandEncoder, src: &wgpu::Texture, dst: &wgpu::Texture) {
    let src_copy_view = src.default_copy_view();
    let dst_copy_view = dst.default_copy_view();
    let copy_size = dst.extent();
    encoder.copy_texture_to_texture(src_copy_view, dst_copy_view, copy_size);
}

/// Returns the directory to save captured frames.
fn capture_directory(app: &App) -> std::path::PathBuf {
    app.project_path()
        .expect("could not locate project_path")
        .join("frames")
}

/// Compiles a shader from the shaders directory
fn compile_shader(
    app: &App,
    device: &wgpu::Device,
    filename: &str,
    kind: shaderc::ShaderKind,
) -> wgpu::ShaderModule {
    let path = app
        .project_path()
        .unwrap()
        .join("src")
        .join("shaders")
        .join(filename)
        .into_os_string()
        .into_string()
        .unwrap();
    let code = fs::read_to_string(path).unwrap();
    let mut compiler = shaderc::Compiler::new().unwrap();
    let spirv = compiler
        .compile_into_spirv(code.as_str(), kind, filename, "main", None)
        .unwrap();
    wgpu::shader_from_spirv_bytes(device, spirv.as_binary_u8())
}

/// writes an image to a textue using a custom shader and render pipeline
fn copy_image_to_texture(
    app: &App,
    window: &Window,
    device: &wgpu::Device,
    image: &DynamicImage,
    texture: &wgpu::Texture,
    sample_count: u32,
    vertex_buffer: &wgpu::Buffer,
) {
    // load and compile shaders
    let vs_mod = compile_shader(app, &device, "shader.vert", shaderc::ShaderKind::Vertex);
    let fs_mod = compile_shader(app, &device, "image.frag", shaderc::ShaderKind::Fragment);

    // prepare textures
    let image_texture = wgpu::Texture::from_image(window, &image);
    let image_texture_view = image_texture.view().build();
    let texture_view = texture.view().build();
    let sampler = wgpu::SamplerBuilder::new().build(device);

    let bind_group_layout = wgpu::BindGroupLayoutBuilder::new()
        .sampled_texture(
            wgpu::ShaderStage::FRAGMENT,
            true,
            wgpu::TextureViewDimension::D2,
            image_texture.component_type(),
        )
        .sampler(wgpu::ShaderStage::FRAGMENT)
        .build(device);

    let bind_group = wgpu::BindGroupBuilder::new()
        .texture_view(&image_texture_view)
        .sampler(&sampler)
        .build(device, &bind_group_layout);

    let pipeline_desc = wgpu::PipelineLayoutDescriptor {
        bind_group_layouts: &[&bind_group_layout],
    };
    let pipeline_layout = device.create_pipeline_layout(&pipeline_desc);

    let render_pipeline =
        create_render_pipeline(device, &pipeline_layout, &vs_mod, &fs_mod, sample_count);

    println!("copying image texture into uniform texture");
    let ce_desc = wgpu::CommandEncoderDescriptor {
        label: Some("texture-renderer"),
    };
    let mut encoder = device.create_command_encoder(&ce_desc);

    // do the render pass
    {
        let mut render_pass = wgpu::RenderPassBuilder::new()
            .color_attachment(&texture_view, |color| color)
            .begin(&mut encoder);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.set_pipeline(&render_pipeline);
        render_pass.set_vertex_buffer(0, &vertex_buffer, 0, 0);
        let vertex_range = 0..VERTICES.len() as u32;
        let instance_range = 0..1;
        render_pass.draw(vertex_range, instance_range);
    }

    window.swap_chain_queue().submit(&[encoder.finish()]);
}
