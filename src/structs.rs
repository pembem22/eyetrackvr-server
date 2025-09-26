use std::time::SystemTime;

use image::{GenericImageView, SubImage};

use crate::camera::Frame;

const EYELID_NEUTRAL_VALUE: f32 = 0.75;

// The plan is to make it possible for this be different for standalone/OpenXR/SteamVR builds.
pub type Timestamp = SystemTime;
pub const ZERO_TIMESTAMP: SystemTime = SystemTime::UNIX_EPOCH;

/// Single eye state.
#[derive(Copy, Clone, Debug)]
pub struct EyeGazeState {
    pub pitch: f32,
    pub yaw: f32,
    pub eyelid: f32,
}

impl Default for EyeGazeState {
    fn default() -> Self {
        Self {
            pitch: 0.0,
            yaw: 0.0,
            eyelid: EYELID_NEUTRAL_VALUE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Eye {
    L,
    R,
}

#[derive(Copy, Clone, Debug)]
pub enum EyesGazeState {
    Mono {
        eye: Eye,
        state: EyeGazeState,
        timestamp: Timestamp,
    },
    Both {
        l_state: EyeGazeState,
        r_state: EyeGazeState,
        timestamp: Timestamp,
    },
}

/// Combined eye gazes with a shared timestamp.
#[derive(Copy, Clone, Debug)]
pub struct CombinedEyeGazeState {
    // For eye expressions.
    // Individual states of each eye with gaze sanitizing so it looks ok.
    pub pitch: f32,
    pub l_yaw: f32,
    pub r_yaw: f32,
    pub l_eyelid: f32,
    pub r_eyelid: f32,

    // Gaze for interaction.
    // Gaze direction without depth, can e.g. ignore one eye if it's closed, etc.
    pub gaze_pitch: f32,
    pub gaze_yaw: f32,
    
    pub timestamp: Timestamp,
}

impl Default for CombinedEyeGazeState {
    fn default() -> Self {
        Self {
            pitch: 0.0,
            l_yaw: 0.0,
            r_yaw: 0.0,
            l_eyelid: EYELID_NEUTRAL_VALUE,
            r_eyelid: EYELID_NEUTRAL_VALUE,

            gaze_pitch: 0.0,
            gaze_yaw: 0.0,

            timestamp: ZERO_TIMESTAMP,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum EyesFrameType {
    Left,
    Rigth,

    // Side-by-side.
    Both,
}

#[derive(Debug, Clone)]
pub struct EyesFrame {
    pub frame_type: EyesFrameType,
    pub frame: Frame,
}

impl EyesFrame {
    pub fn get_left_view(&self) -> Option<SubImage<&image::ImageBuffer<image::Rgb<u8>, Vec<u8>>>> {
        match self.frame_type {
            EyesFrameType::Left => {
                let decoded = &self.frame.decoded;
                Some(decoded.view(0, 0, decoded.width(), decoded.height()))
            }
            EyesFrameType::Rigth => None,
            EyesFrameType::Both => {
                let decoded = &self.frame.decoded;
                Some(decoded.view(0, 0, decoded.width() / 2, decoded.height()))
            }
        }
    }

    pub fn get_right_view(&self) -> Option<SubImage<&image::ImageBuffer<image::Rgb<u8>, Vec<u8>>>> {
        match self.frame_type {
            EyesFrameType::Left => None,
            EyesFrameType::Rigth => {
                let decoded = &self.frame.decoded;
                Some(decoded.view(0, 0, decoded.width(), decoded.height()))
            }
            EyesFrameType::Both => {
                let decoded = &self.frame.decoded;
                Some(decoded.view(
                    decoded.width() / 2,
                    0,
                    decoded.width() / 2,
                    decoded.height(),
                ))
            }
        }
    }
}
