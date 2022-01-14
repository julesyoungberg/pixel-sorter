use nannou::{prelude::*, wgpu::TextureSnapshot};

pub struct FrameCapturer {
    texture_capturer: wgpu::TextureCapturer,
    snapshot: Option<TextureSnapshot>,
}

impl FrameCapturer {
    pub fn new(app: &App) -> Self {
        let texture_capturer = wgpu::TextureCapturer::default();

        std::fs::create_dir_all(&capture_directory(app)).unwrap();

        Self {
            texture_capturer,
            snapshot: None,
        }
    }

    pub fn take_snapshot(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
    ) {
        // Take a snapshot of the texture. The capturer will do the following:
        //
        // 1. Resolve the texture to a non-multisampled texture if necessary.
        // 2. Convert the format to non-linear 8-bit sRGBA ready for image storage.
        // 3. Copy the result to a buffer ready to be mapped for reading.
        self.snapshot = Some(self.texture_capturer.capture(device, encoder, &texture));
    }

    pub fn save_frame(&mut self, app: &App) {
        // Submit a function for writing our snapshot to a PNG.
        //
        // NOTE: It is essential that the commands for capturing the snapshot are `submit`ted before we
        // attempt to read the snapshot - otherwise we will read a blank texture!
        let elapsed_frames = app.main_window().elapsed_frames();
        let path = capture_directory(app)
            .join(elapsed_frames.to_string())
            .with_extension("png");

        if let Some(snapshot) = self.snapshot.take() {
            snapshot
                .read(move |result| {
                    let image = result.expect("failed to map texture memory");
                    image
                        .save(&path)
                        .expect("failed to save texture to png image");
                })
                .unwrap();
        }
    }
}

/// Returns the directory to save captured frames.
fn capture_directory(app: &App) -> std::path::PathBuf {
    app.project_path()
        .expect("could not locate project_path")
        .join("frames")
}
