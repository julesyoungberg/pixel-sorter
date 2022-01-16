use glsl_layout::float;
use glsl_layout::*;
use nannou::image::GenericImageView;
use nannou::image::{self, DynamicImage};
use nannou::prelude::*;
use std::fs;

mod capture;
mod geometry;
mod render;

use crate::capture::*;
use crate::geometry::*;
use crate::render::*;

struct Model {
    width: u32,
    height: u32,
    uniform_texture: wgpu::Texture,
    frame_capturer: FrameCapturer,
    field_generator: CustomRenderer,
    sorter: CustomRenderer,
    vertex_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
}

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
    let image_path = app.assets_path().unwrap().join("industial.jpeg");
    let image = image::open(image_path).unwrap();
    let (width, height) = image.dimensions();
    let scale = 1;

    let window_id = app
        .new_window()
        .size(width, height)
        .view(view)
        .build()
        .unwrap();
    let window = app.window(window_id).unwrap();
    let device = window.swap_chain_device();
    let sample_count = window.msaa_samples();

    println!("image dimensions: {}, {}", width, height);

    let swidth = width / scale;
    let sheight = height / scale;

    println!("scaled dimensions: {}, {}", swidth, sheight);

    // Create the compute shader module.
    println!("loading shaders");
    let vs_mod = compile_shader(app, &device, "shader.vert", shaderc::ShaderKind::Vertex);
    let field_fs_mod = compile_shader(app, &device, "field.frag", shaderc::ShaderKind::Fragment);
    let sort_fs_mod = compile_shader(app, &device, "sort.frag", shaderc::ShaderKind::Fragment);

    let uniform_texture = render::create_app_texture(&device, swidth, sheight, 1);

    // Create the sampler for sampling from the source texture.
    let sampler = wgpu::SamplerBuilder::new().build(device);

    // create uniform buffer
    let uniforms = create_uniforms(swidth, sheight, 0);
    println!("uniforms: {:?}", uniforms);
    let uniforms_bytes = uniforms_as_bytes(&uniforms);
    let uniform_buffer = device.create_buffer_with_data(
        uniforms_bytes,
        wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
    );

    let field_generator = CustomRenderer::new::<Uniforms>(
        &device,
        &vs_mod,
        &field_fs_mod,
        None,
        None,
        Some(&uniform_buffer),
        swidth,
        sheight,
        sample_count,
    );

    let sorter_uniform_textures = vec![&uniform_texture, &field_generator.output_texture];

    let sorter = CustomRenderer::new::<Uniforms>(
        &device,
        &vs_mod,
        &sort_fs_mod,
        Some(&sorter_uniform_textures),
        Some(&sampler),
        Some(&uniform_buffer),
        swidth,
        sheight,
        sample_count,
    );

    println!("creating vertex buffer");
    let vertices_bytes = vertices_as_bytes(&VERTICES[..]);
    let vertex_buffer = device.create_buffer_with_data(vertices_bytes, wgpu::BufferUsage::VERTEX);

    copy_image_to_texture(
        app,
        &window,
        &device,
        &image,
        &uniform_texture,
        1,
        &vertex_buffer,
    );

    let frame_capturer = FrameCapturer::new(app);

    Model {
        width,
        height,
        uniform_texture,
        frame_capturer,
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

    model
        .frame_capturer
        .take_snapshot(device, &mut encoder, &model.uniform_texture);

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

    model.frame_capturer.save_frame(app);

    // slow it down just a bit
    std::thread::sleep(std::time::Duration::from_millis(50));
}

fn view(_app: &App, model: &Model, frame: Frame) {
    // Sample the texture and write it to the frame.
    let mut encoder = frame.command_encoder();
    model
        // .field_generator
        .sorter
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
        render::create_render_pipeline(device, &pipeline_layout, &vs_mod, &fs_mod, sample_count);

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
