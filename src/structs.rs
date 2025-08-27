use std::time::SystemTime;

#[derive(Copy, Clone, Debug)]
pub struct EyeState {
    pub pitch: f32,
    pub yaw: f32,
    pub eyelid: f32,
    pub timestamp: SystemTime,
}

impl Default for EyeState {
    fn default() -> Self {
        Self {
            pitch: 0.0,
            yaw: 0.0,
            eyelid: 0.75,
            timestamp: SystemTime::UNIX_EPOCH,
        }
    }
}
