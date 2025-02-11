use std::time::{Duration, SystemTime};

use async_broadcast::Receiver;
use image::DynamicImage;
use imgui::TextureId;
use imgui_wgpu::{Texture, TextureConfig};

use crate::{ui, Frame, CAMERA_FRAME_SIZE};

#[derive(Clone, Copy)]
pub struct CameraTexture {
    last_delta: Duration,
    last_timestamp: SystemTime,
    texture_id: imgui::TextureId,
}

impl CameraTexture {
    pub fn new(ui: &mut ui::UI, label: Option<&str>) -> CameraTexture {
        let texture_config: TextureConfig<'_> = TextureConfig {
            size: wgpu::Extent3d {
                width: CAMERA_FRAME_SIZE,
                height: CAMERA_FRAME_SIZE,
                ..Default::default()
            },
            label,
            format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            ..Default::default()
        };

        let texture = Texture::new(&ui.device, &ui.renderer, texture_config);

        CameraTexture {
            last_delta: Duration::ZERO,
            last_timestamp: SystemTime::now(),
            texture_id: ui.renderer.textures.insert(texture),
        }
    }

    pub fn update_texture(
        &mut self,
        rx: &mut Receiver<Frame>,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) {
        let frame = match rx.try_recv() {
            Ok(frame) => frame,
            Err(_) => return,
        };

        let image = DynamicImage::from(frame.decoded.clone()).into_rgba8();

        // let expand_range = |v: u8| {
        //     println!("{}", v);
        //     (((v - 15) as u32) * 255 / (235 - 15 + 1)) as u8
        // };
        // image.pixels_mut().for_each(|p| {
        //     p.0[0..3].iter_mut().for_each(|p| {*p = expand_range(*p)});
        // });

        renderer.textures.get(self.texture_id).unwrap().write(
            queue,
            &image,
            CAMERA_FRAME_SIZE,
            CAMERA_FRAME_SIZE,
        );

        self.last_delta = frame.timestamp.duration_since(self.last_timestamp).unwrap();
        self.last_timestamp = frame.timestamp;
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

    pub fn get_texture_id(self) -> TextureId {
        self.texture_id
    }

    pub fn get_fps(self) -> f32 {
        1.0 / self.last_delta.as_secs_f32()
    }
}
