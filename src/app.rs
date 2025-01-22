use async_broadcast::Sender;
use tokio::task::JoinHandle;

use crate::Frame;
use crate::{Camera, Eye};

pub(crate) struct App {
    l_camera: Camera,
    r_camera: Camera,
    f_camera: Camera,
}

impl App {
    pub fn new(l_sender: Sender<Frame>, r_sender: Sender<Frame>, f_sender: Sender<Frame>) -> App {
        let l_camera = Camera::new(Eye::L, l_sender);
        let r_camera = Camera::new(Eye::R, r_sender);
        let f_camera = Camera::new(Eye::R, f_sender);

        App { l_camera, r_camera, f_camera }
    }

    pub fn start_cameras(
        &mut self,
        l_tty_path: String,
        r_tty_path: String,
        f_tty_path: String,
    ) -> tokio_serial::Result<(JoinHandle<()>, JoinHandle<()>, JoinHandle<()>)> {
        Ok((
            self.l_camera.start(l_tty_path)?,
            self.r_camera.start(r_tty_path)?,
            self.f_camera.start(f_tty_path)?,
        ))
    }
}
