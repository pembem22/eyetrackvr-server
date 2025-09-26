use std::{
    sync::{Mutex, OnceLock},
    time::SystemTime,
};

use async_broadcast::{InactiveReceiver, Receiver};

use crate::structs::CombinedEyeGazeState;

pub static OPENXR_OUTPUT_BRIDGE: OnceLock<Mutex<OpenXROutputBridge>> = OnceLock::new();

pub struct OpenXROutputBridge {
    receiver: Receiver<CombinedEyeGazeState>,
    last_state: Option<CombinedEyeGazeState>,
}

impl OpenXROutputBridge {
    fn new(receiver: &InactiveReceiver<CombinedEyeGazeState>) -> Self {
        Self {
            receiver: receiver.activate_cloned(),
            last_state: None,
        }
    }

    pub fn get_eyes_state(&mut self) -> Option<CombinedEyeGazeState> {
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

pub fn start_openxr_output(receiver: &InactiveReceiver<CombinedEyeGazeState>) {
    OPENXR_OUTPUT_BRIDGE.get_or_init(|| Mutex::new(OpenXROutputBridge::new(receiver)));
}
