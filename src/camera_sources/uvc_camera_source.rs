use std::io::Cursor;
use std::time::SystemTime;

use crate::camera::Frame;
use crate::camera_dispatcher::CameraDispatcher;
use crate::camera_sources::{CameraSource, FpsCounter};

use nokhwa::utils::CameraFormat;
use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
};

#[derive(Clone, Debug)]
pub struct UvcCameraSource {
    uvc_index: u32,
}

impl UvcCameraSource {
    pub fn new(uvc_index: u32) -> Self {
        Self { uvc_index }
    }
}

impl CameraSource for UvcCameraSource {
    fn run(&self, dispatcher: Box<dyn CameraDispatcher>) -> tokio::task::JoinHandle<()> {
        let uvc_index = self.uvc_index;
        let mut fps = FpsCounter::new();

        let future = move || {
            let index = CameraIndex::Index(uvc_index);
            let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Exact(
                CameraFormat::new_from(320, 240, nokhwa::utils::FrameFormat::MJPEG, 120),
            ));
            println!("Requested format: {requested:?}");
            let mut camera = nokhwa::Camera::new(index, requested).unwrap();

            // Docs say this is required, but not calling it also works lmao whatever, that lib is cooked.
            camera.open_stream().unwrap();

            println!(
                "Connected to a UVC camera fps:{} {:?}",
                camera.frame_rate(),
                camera.frame_format()
            );

            while let Ok(frame_raw) = camera.frame_raw() {
                let mut decoder = image::ImageReader::new(Cursor::new(&frame_raw));
                decoder.set_format(image::ImageFormat::Jpeg);

                let image = decoder.decode();

                if image.is_err() {
                    println!("Failed to decode image");
                    continue;
                }

                let frame = Frame {
                    timestamp: SystemTime::now(),
                    raw_jpeg_data: Some(Vec::from(frame_raw)),
                    decoded: image.unwrap().into_rgb8(),
                };

                dispatcher.dispatch(frame).block_on();

                fps.update_fps();
            }
        };
        tokio::task::spawn_blocking(future)
    }
}
