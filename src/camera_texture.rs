use async_broadcast::Receiver;
use image::DynamicImage;
use imgui_wgpu::{Texture, TextureConfig};

use crate::{ui, Frame, CAMERA_FRAME_SIZE};

#[derive(Clone, Copy)]
pub struct CameraTexture {
    texture_id: imgui::TextureId,
}

impl CameraTexture {
    pub fn new(ui: &mut ui::UI) -> CameraTexture {
        let texture_config: TextureConfig<'_> = TextureConfig {
            size: wgpu::Extent3d {
                width: CAMERA_FRAME_SIZE,
                height: CAMERA_FRAME_SIZE,
                ..Default::default()
            },
            label: Some("lenna texture"),
            format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            ..Default::default()
        };

        let texture = Texture::new(&ui.device, &ui.renderer, texture_config);

        CameraTexture {
            texture_id: ui.renderer.textures.insert(texture),
        }
    }

    pub fn update_texture(
        self,
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
    }

    pub fn build(self, ui: &imgui::Ui) {
        imgui::Image::new(
            self.texture_id,
            [CAMERA_FRAME_SIZE as f32, CAMERA_FRAME_SIZE as f32],
        )
        .build(ui);
    }
}
