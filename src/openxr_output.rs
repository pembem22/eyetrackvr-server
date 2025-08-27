use std::{
    sync::{Mutex, OnceLock},
    time::SystemTime,
};

use async_broadcast::{InactiveReceiver, Receiver};

use crate::structs::EyeState;

#[derive(Debug, Copy, Clone)]
pub struct OpenXRGaze {
    // Ideally we have synced cameras and a single timestamp...
    pub l_timestamp: SystemTime,
    pub r_timestamp: SystemTime,

    // In radians.
    pub pitch: f32,
    // In radians.
    pub yaw: f32,
}

impl OpenXRGaze {
    pub fn latest_timestamp(&self) -> SystemTime {
        std::cmp::max(self.l_timestamp, self.r_timestamp)
    }
}

pub static OPENXR_OUTPUT_BRIDGE: OnceLock<Mutex<OpenXROutputBridge>> = OnceLock::new();

pub struct OpenXROutputBridge {
    receiver: Receiver<(EyeState, EyeState)>,
    last_state: Option<(EyeState, EyeState)>,
    last_openxr_gaze: Option<OpenXRGaze>,
}

impl OpenXROutputBridge {
    fn new(receiver: &InactiveReceiver<(EyeState, EyeState)>) -> Self {
        Self {
            receiver: receiver.activate_cloned(),
            last_state: None,
            last_openxr_gaze: None,
        }
    }

    pub fn get_openxr_gaze(&mut self) -> Option<OpenXRGaze> {
        let Some((l_state, r_state)) = self.get_state() else {
            return self.last_openxr_gaze;
        };

        let gaze = OpenXRGaze {
            l_timestamp: l_state.timestamp,
            r_timestamp: r_state.timestamp,

            pitch: ((l_state.pitch + r_state.pitch) / 2.0).to_radians(),
            yaw: ((l_state.yaw + r_state.yaw) / 2.0).to_radians(),
        };

        self.last_openxr_gaze = Some(gaze);

        self.last_openxr_gaze
    }

    pub fn get_state(&mut self) -> Option<(EyeState, EyeState)> {
        let state = loop {
            match self.receiver.try_recv() {
                Ok(state) => break state,
                Err(err) => match err {
                    async_broadcast::TryRecvError::Overflowed(_) => continue,
                    async_broadcast::TryRecvError::Closed
                    | async_broadcast::TryRecvError::Empty => return self.last_state,
                },
            };
        };

        self.last_state = Some(state);

        self.last_state
    }
}

pub fn start_openxr_output(receiver: &InactiveReceiver<(EyeState, EyeState)>) {
    OPENXR_OUTPUT_BRIDGE.get_or_init(|| Mutex::new(OpenXROutputBridge::new(receiver)));
}
