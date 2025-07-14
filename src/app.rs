use async_broadcast::{InactiveReceiver, Sender};
use tokio::task::JoinHandle;

use crate::Frame;
use crate::inference::EyeState;
use crate::{Camera, Eye};

// Utility for creating a broadcast pair with 1 element queue, overflow on, and deactivated receiver.
pub fn broadcast<T>() -> (Sender<T>, InactiveReceiver<T>) {
    let (tx, mut rx) = async_broadcast::broadcast::<T>(1);
    rx.set_overflow(true);
    (tx, rx.deactivate())
}

// Contains all the elements and senders/receivers.
pub(crate) struct App {
    pub l_camera: Camera,
    pub r_camera: Camera,
    pub f_camera: Camera,

    // Camera frames
    pub l_cam_tx: Sender<Frame>,
    pub l_cam_rx: InactiveReceiver<Frame>,
    pub r_cam_tx: Sender<Frame>,
    pub r_cam_rx: InactiveReceiver<Frame>,
    pub f_cam_tx: Sender<Frame>,
    pub f_cam_rx: InactiveReceiver<Frame>,

    // Inference
    pub l_raw_eye_tx: Sender<EyeState>,
    pub l_raw_eye_rx: InactiveReceiver<EyeState>,
    pub r_raw_eye_tx: Sender<EyeState>,
    pub r_raw_eye_rx: InactiveReceiver<EyeState>,

    // Gaze processing
    pub l_filtered_eye_tx: Sender<EyeState>,
    pub l_filtered_eye_rx: InactiveReceiver<EyeState>,
    pub r_filtered_eye_tx: Sender<EyeState>,
    pub r_filtered_eye_rx: InactiveReceiver<EyeState>,

    // Combine gaze
    // TODO: Merge the above and this into one, produce CombinedGazeState struct.
    pub filtered_eyes_tx: Sender<(EyeState, EyeState)>,
    pub filtered_eyes_rx: InactiveReceiver<(EyeState, EyeState)>,
}

impl App {
    pub fn new() -> App {
        // Camera channels

        let (l_cam_tx, l_cam_rx) = broadcast::<Frame>();
        let (r_cam_tx, r_cam_rx) = broadcast::<Frame>();
        let (f_cam_tx, f_cam_rx) = broadcast::<Frame>();

        // Inference channels

        let (l_raw_eye_tx, l_raw_eye_rx) = broadcast::<EyeState>();
        let (r_raw_eye_tx, r_raw_eye_rx) = broadcast::<EyeState>();

        // Gaze processing channels

        let (l_filtered_eye_tx, l_filtered_eye_rx) = broadcast::<EyeState>();
        let (r_filtered_eye_tx, r_filtered_eye_rx) = broadcast::<EyeState>();

        // Combine gaze channels

        let (filtered_eyes_tx, filtered_eyes_rx) = broadcast::<(EyeState, EyeState)>();

        App {
            l_camera: Camera::new(Eye::L, l_cam_tx.clone()),
            r_camera: Camera::new(Eye::R, r_cam_tx.clone()),
            f_camera: Camera::new(Eye::R, f_cam_tx.clone()),

            l_cam_tx,
            l_cam_rx,
            r_cam_tx,
            r_cam_rx,
            f_cam_tx,
            f_cam_rx,

            l_raw_eye_tx,
            l_raw_eye_rx,
            r_raw_eye_tx,
            r_raw_eye_rx,

            l_filtered_eye_tx,
            l_filtered_eye_rx,
            r_filtered_eye_tx,
            r_filtered_eye_rx,

            filtered_eyes_tx,
            filtered_eyes_rx,
        }
    }

    pub fn start_cameras(
        &self,
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
