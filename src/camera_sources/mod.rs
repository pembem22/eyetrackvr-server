use std::time::SystemTime;

use crate::camera_dispatcher::CameraDispatcher;

#[cfg(feature = "desktop")]
mod uvc_camera_source;
#[cfg(feature = "desktop")]
pub use uvc_camera_source::UvcCameraSource;

mod http_camera_source;
pub use http_camera_source::HttpCameraSource;

pub trait CameraSource {
    fn run(&self, dispatcher: Box<dyn CameraDispatcher>) -> tokio::task::JoinHandle<()>;
}

#[derive(Clone, Debug)]
struct FpsCounter {
    last_second: SystemTime,
    frames_since_last_second: u32,
}

impl FpsCounter {
    fn new() -> Self {
        Self {
            last_second: SystemTime::now(),
            frames_since_last_second: 0,
        }
    }

    fn update_fps(&mut self) {
        let now = SystemTime::now();
        if now.duration_since(self.last_second).unwrap().as_secs() > 0 {
            println!("FPS: {}", self.frames_since_last_second);

            self.last_second = now;
            self.frames_since_last_second = 0;
        }

        self.frames_since_last_second += 1;
    }
}
