use async_broadcast::Sender;
use tokio::task::JoinHandle;

use crate::Frame;
use crate::{Camera, Eye};

pub(crate) struct App {
    l_camera: Camera,
    r_camera: Camera,
}

impl App {
    pub fn new(l_sender: Sender<Frame>, r_sender: Sender<Frame>) -> App {
        let l_camera = Camera::new(Eye::L, l_sender);
        let r_camera = Camera::new(Eye::R, r_sender);

        App { l_camera, r_camera }
    }

    pub fn start_cameras(
        &mut self,
        l_tty_path: String,
        r_tty_path: String,
    ) -> tokio_serial::Result<(JoinHandle<()>, JoinHandle<()>)> {
        Ok((
            self.l_camera.start(l_tty_path)?,
            self.r_camera.start(r_tty_path)?,
        ))
    }
}
