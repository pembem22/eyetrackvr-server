use std::fmt::Debug;

use async_broadcast::Sender;
use async_trait::async_trait;

use crate::{
    camera::Frame,
    structs::{Eye, EyesFrame, EyesFrameType},
};

#[async_trait]
pub trait CameraDispatcher: Debug + Send {
    async fn dispatch(&self, frame: Frame);
}

#[derive(Debug)]
pub struct StereoEyesCameraDispatcher {
    sender: Sender<EyesFrame>,
}

impl StereoEyesCameraDispatcher {
    pub fn new(sender: Sender<EyesFrame>) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl CameraDispatcher for StereoEyesCameraDispatcher {
    async fn dispatch(&self, frame: Frame) {
        self.sender
            .broadcast_direct(EyesFrame {
                frame,
                frame_type: EyesFrameType::Both,
            })
            .await
            .unwrap();
    }
}

#[derive(Debug)]
pub struct MonoEyeCameraDispatcher {
    sender: Sender<EyesFrame>,
    eye: Eye,
}

impl MonoEyeCameraDispatcher {
    pub fn new(eye: Eye, sender: Sender<EyesFrame>) -> Self {
        Self { sender, eye }
    }
}

#[async_trait]
impl CameraDispatcher for MonoEyeCameraDispatcher {
    async fn dispatch(&self, frame: Frame) {
        self.sender
            .broadcast_direct(EyesFrame {
                frame,
                frame_type: match self.eye {
                    Eye::L => EyesFrameType::Left,
                    Eye::R => EyesFrameType::Rigth,
                },
            })
            .await
            .unwrap();
    }
}

#[derive(Debug)]
pub struct MonoCameraDispatcher {
    sender: Sender<Frame>,
}

impl MonoCameraDispatcher {
    pub fn new(sender: Sender<Frame>) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl CameraDispatcher for MonoCameraDispatcher {
    async fn dispatch(&self, frame: Frame) {
        self.sender.broadcast_direct(frame).await.unwrap();
    }
}
