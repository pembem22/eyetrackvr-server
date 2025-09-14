use std::time::SystemTime;

use image::{GenericImageView, SubImage};

use crate::camera::Frame;

const EYELID_OPEN_VALUE: f32 = 0.75;

/// Single eye state.
#[derive(Copy, Clone, Debug)]
pub struct EyeGazeState {
    pub pitch: f32,
    pub yaw: f32,
    pub eyelid: f32,
    pub timestamp: SystemTime,
}

impl Default for EyeGazeState {
    fn default() -> Self {
        Self {
            pitch: 0.0,
            yaw: 0.0,
            eyelid: EYELID_OPEN_VALUE,
            timestamp: SystemTime::UNIX_EPOCH,
        }
    }
}

/// Both eyes states together, but not guaranteed to have identical timestamps.
#[derive(Copy, Clone, Debug, Default)]
pub struct EyesGazeState {
    pub l: EyeGazeState,
    pub r: EyeGazeState,
}

/// Combined eye gazes with a shared timestamp.
#[derive(Copy, Clone, Debug)]
pub struct CombinedEyeGazeState {
    pub pitch: f32,
    pub l_yaw: f32,
    pub r_yaw: f32,
    // TODO: separate eye expressions from gaze.
    pub l_eyelid: f32,
    pub r_eyelid: f32,
    pub timestamp: SystemTime,
}

impl Default for CombinedEyeGazeState {
    fn default() -> Self {
        Self {
            pitch: 0.0,
            l_yaw: 0.0,
            r_yaw: 0.0,
            l_eyelid: EYELID_OPEN_VALUE,
            r_eyelid: EYELID_OPEN_VALUE,
            timestamp: SystemTime::UNIX_EPOCH,
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
