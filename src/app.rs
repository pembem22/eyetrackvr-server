use async_broadcast::{InactiveReceiver, Sender};
use tokio::task::JoinHandle;

use crate::camera::{Camera, Eye, Frame};
use crate::structs::{CombinedEyeGazeState, EyesFrame, EyesGazeState};

// Utility for creating a broadcast pair with 1 element queue, overflow on, and deactivated receiver.
pub fn inactive_broadcast<T>() -> (Sender<T>, InactiveReceiver<T>) {
    let (tx, mut rx) = async_broadcast::broadcast::<T>(1);
    rx.set_overflow(true);
    (tx, rx.deactivate())
}

// Contains all the elements and senders/receivers.
pub(crate) struct App {
    // Eye tracking camera(s).
    pub eye_cam_tx: Sender<EyesFrame>,
    pub eyes_cam_rx: InactiveReceiver<EyesFrame>,

    // Face tracking camera.
    pub f_cam_tx: Sender<Frame>,
    pub f_cam_rx: InactiveReceiver<Frame>,

    // Inference.
    pub raw_eyes_tx: Sender<EyesGazeState>,
    pub raw_eyes_rx: InactiveReceiver<EyesGazeState>,

    // Combined gaze.
    pub combined_eyes_tx: Sender<CombinedEyeGazeState>,
    pub combined_eyes_rx: InactiveReceiver<CombinedEyeGazeState>,
}

impl App {
    pub fn new() -> App {
        // Eye channel

        let (eye_cam_tx, eye_cam_rx) = inactive_broadcast::<EyesFrame>();

        // Face channel

        let (f_cam_tx, f_cam_rx) = inactive_broadcast::<Frame>();

        // Inference channels

        let (raw_eyes_tx, raw_eyes_rx) = inactive_broadcast::<EyesGazeState>();

        // Gaze processing channels

        let (combined_eyes_tx, combined_eyes_rx) = inactive_broadcast::<CombinedEyeGazeState>();

        App {
            eye_cam_tx,
            eyes_cam_rx: eye_cam_rx,

            f_cam_tx,
            f_cam_rx,

            raw_eyes_tx,
            raw_eyes_rx,

            combined_eyes_tx,
            combined_eyes_rx,
        }
    }
}
