use std::time::{Duration, SystemTime};

use async_broadcast::Receiver;
use image::DynamicImage;

use crate::camera::{CAMERA_FRAME_SIZE, Frame};

#[derive(Clone, Copy)]
pub struct CameraTexture {
    last_delta: Duration,
    last_timestamp: SystemTime,
    texture_id: imgui::TextureId,

    last_second: SystemTime,
    frames_since_last_second: u32,
    last_fps: u32,
}

impl CameraTexture {
    pub fn new(
        device: &mut wgpu::Device,
        renderer: &mut imgui_wgpu::Renderer,
        label: Option<&str>,
    ) -> CameraTexture {
        let texture_config = imgui_wgpu::TextureConfig {
            size: wgpu::Extent3d {
                width: CAMERA_FRAME_SIZE,
                height: CAMERA_FRAME_SIZE,
                ..Default::default()
            },
            label,
            format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            ..Default::default()
        };

        let texture = imgui_wgpu::Texture::new(device, renderer, texture_config);

        CameraTexture {
            last_delta: Duration::ZERO,
            last_timestamp: SystemTime::now(),
            texture_id: renderer.textures.insert(texture),

            last_second: SystemTime::now(),
            frames_since_last_second: 0,
            last_fps: 0,
        }
    }

    pub fn update_texture(
        &mut self,
        rx: &mut Receiver<Frame>,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) {
        let frame = loop {
            match rx.try_recv() {
                Ok(frame) => break frame,
                Err(err) => match err {
                    async_broadcast::TryRecvError::Overflowed(_) => continue,
                    async_broadcast::TryRecvError::Closed
                    | async_broadcast::TryRecvError::Empty => return,
                },
            };
        };

        let image = DynamicImage::from(frame.decoded.clone()).into_rgba8();

        renderer.textures.get(self.texture_id).unwrap().write(
            queue,
            &image,
            CAMERA_FRAME_SIZE,
            CAMERA_FRAME_SIZE,
        );

        // TODO: why crashes here on Android?
        self.last_delta = frame
            .timestamp
            .duration_since(self.last_timestamp)
            .unwrap_or_default();
        self.last_timestamp = frame.timestamp;

        self.update_fps();
        self.frames_since_last_second += 1;
    }

    pub fn build(self, ui: &imgui::Ui) {
        imgui::Image::new(
            self.texture_id,
            [CAMERA_FRAME_SIZE as f32, CAMERA_FRAME_SIZE as f32],
        )
        .uv0([1.0, 0.0])
        .uv1([0.0, 1.0])
        .build(ui);
    }

    pub fn get_texture_id(self) -> imgui::TextureId {
        self.texture_id
    }

    pub fn get_fps(&self) -> u32 {
        // Can't do this here, cause self needs to be mutable,
        // which breaks stuff in imgui closures.
        // self.update_fps();
        self.last_fps
    }

    fn update_fps(&mut self) {
        let now = SystemTime::now();
        if now.duration_since(self.last_second).unwrap().as_secs() > 0 {
            self.last_fps = self.frames_since_last_second;

            self.last_second = now;
            self.frames_since_last_second = 0;
        }
    }
}
