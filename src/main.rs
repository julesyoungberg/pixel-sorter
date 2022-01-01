use glsl_layout::float;
use glsl_layout::*;
use nannou::prelude::*;

struct Model {
    width: u32,
    height: u32,
    app_texture: wgpu::Texture,
    uniform_texture: wgpu::Texture,
    renderer: nannou::draw::Renderer,
    texture_capturer: wgpu::TextureCapturer,
    texture_reshaper: wgpu::TextureReshaper,
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
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
    let image_path = app
        .assets_path()
        .unwrap()
        .join("images")
        .join("vqgan_graffiti.png");
    let image = image::open(logo_path).unwrap();
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
    let vs_mod = wgpu::shader_from_spirv_bytes(device, include_bytes!("shaders/vert.spv"));
    let fs_mod = wgpu::shader_from_spirv_bytes(device, include_bytes!("shaders/frag.spv"));

    println!("creating uniform texture");
    let uniform_texture = wgpu::Texture::from_image(&window, &image);
    let uniform_texture_view = uniform_texture.view().build();

    // Create the sampler for sampling from the source texture.
    println!("creating sampler");
    let sampler = wgpu::SamplerBuilder::new().build(device);

    // create uniform buffer
    let uniforms = create_uniforms(width, height, 0);
    println!("uniforms: {:?}", uniforms);
    let uniforms_bytes = uniforms_as_bytes(&uniforms);
    let uniform_buffer = device.create_buffer_with_data(
        uniforms_bytes,
        wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
    );

    // create our custom texture for rendering
    println!("creating app texure and reshaper");
    let app_texture = create_app_texture(&device, width, height, sample_count);
    let texture_reshaper = create_texture_reshaper(&device, &app_texture, sample_count);

    println!("creating bind group layout");
    let bind_group_layout = create_bind_group_layout(device, uniform_texture_view.component_type());
    println!("creating bind group");
    let bind_group = create_bind_group(
        device,
        &bind_group_layout,
        &uniform_texture_view,
        &sampler,
        &uniform_buffer,
    );
    println!("creating pipeline layout");
    let pipeline_layout = create_pipeline_layout(device, &bind_group_layout);
    println!("creating render pipeline");
    let render_pipeline =
        create_render_pipeline(device, &pipeline_layout, &vs_mod, &fs_mod, sample_count);

    println!("creating vertex buffer");
    let vertices_bytes = vertices_as_bytes(&VERTICES[..]);
    let vertex_buffer = device.create_buffer_with_data(vertices_bytes, wgpu::BufferUsage::VERTEX);

    Model {
        width,
        height,
        app_texture,
        uniform_texture,
        renderer,
        texture_capturer,
        texture_reshaper,
        bind_group,
        render_pipeline,
        vertex_buffer,
        uniform_buffer,
    }
}

fn update(app: &App, model: &mut Model, _update: Update) {
    let window = app.main_window();
    let device = window.swap_chain_device();
    let texture_view = model.app_texture.view().build();

    // An update for the uniform buffer with the current time.
    let elapsed_frames = app.main_window().elapsed_frames();
    let uniforms = create_uniforms(width, height, elapsed_frames % 2);
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

    {
        let mut render_pass = wgpu::RenderPassBuilder::new()
            .color_attachment(&texture_view, |color| color)
            .begin(&mut encoder);
        render_pass.set_bind_group(0, &model.bind_group, &[]);
        render_pass.set_pipeline(&model.render_pipeline);
        render_pass.set_vertex_buffer(0, &model.vertex_buffer, 0, 0);
        let vertex_range = 0..VERTICES.len() as u32;
        let instance_range = 0..1;
        render_pass.draw(vertex_range, instance_range);
    }

    // copy app texture to uniform texture
    copy_texture(&mut encoder, &model.app_texture, &model.uniform_texture);

    // Take a snapshot of the texture. The capturer will do the following:
    //
    // 1. Resolve the texture to a non-multisampled texture if necessary.
    // 2. Convert the format to non-linear 8-bit sRGBA ready for image storage.
    // 3. Copy the result to a buffer ready to be mapped for reading.
    // let snapshot = model
    //     .texture_capturer
    //     .capture(device, &mut encoder, &model.uniform_texture);

    // submit encoded command buffer
    window.swap_chain_queue().submit(&[encoder.finish()]);

    // Submit a function for writing our snapshot to a PNG.
    //
    // NOTE: It is essential that the commands for capturing the snapshot are `submit`ted before we
    // attempt to read the snapshot - otherwise we will read a blank texture!
    // let elapsed_frames = app.main_window().elapsed_frames();
    // let path = capture_directory(app)
    //     .join(elapsed_frames.to_string())
    //     .with_extension("png");
    // snapshot
    //     .read(move |result| {
    //         let image = result.expect("failed to map texture memory");
    //         image
    //             .save(&path)
    //             .expect("failed to save texture to png image");
    //     })
    //     .unwrap();
}

fn view(app: &App, model: &Model, frame: Frame) {
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
    msaa_samples: u32,
) -> wgpu::TextureReshaper {
    let texture_view = texture.view().build();
    let texture_component_type = texture.component_type();
    let dst_format = Frame::TEXTURE_FORMAT;
    wgpu::TextureReshaper::new(
        device,
        &texture_view,
        msaa_samples,
        texture_component_type,
        msaa_samples,
        dst_format,
    )
}

fn create_bind_group_layout(
    device: &wgpu::Device,
    texture_component_type: wgpu::TextureComponentType,
) -> wgpu::BindGroupLayout {
    let storage_dynamic = false;
    let storage_readonly = false;
    let uniform_dynamic = false;
    wgpu::BindGroupLayoutBuilder::new()
        .sampled_texture(
            wgpu::ShaderStage::FRAGMENT,
            true,
            wgpu::TextureViewDimension::D2,
            texture_component_type,
        )
        .sampler(wgpu::ShaderStage::FRAGMENT)
        .uniform_buffer(wgpu::ShaderStage::FRAGMENT, uniform_dynamic)
        .build(device)
}

fn create_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    texture: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    uniform_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    wgpu::BindGroupBuilder::new()
        .texture_view(texture)
        .sampler(sampler)
        .buffer::<Uniforms>(uniform_buffer, 0..1)
        .build(device, layout)
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

// See the `nannou::wgpu::bytes` documentation for why this is necessary.
fn vertices_as_bytes(data: &[Vertex]) -> &[u8] {
    unsafe { wgpu::bytes::from_slice(data) }
}

pub fn copy_texture(encoder: &mut wgpu::CommandEncoder, src: &wgpu::Texture, dst: &wgpu::Texture) {
    let src_copy_view = src.default_copy_view();
    let dst_copy_view = dst.default_copy_view();
    let copy_size = dst.extent();
    encoder.copy_texture_to_texture(src_copy_view, dst_copy_view, copy_size);
}

// The directory where we'll save the frames.
fn capture_directory(app: &App) -> std::path::PathBuf {
    app.project_path()
        .expect("could not locate project_path")
        .join("frames")
}
