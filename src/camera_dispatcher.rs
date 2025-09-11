use std::fmt::Debug;

use async_broadcast::Sender;
use async_trait::async_trait;
use futures::future::join_all;
use image::GenericImageView;

use crate::camera::Frame;

#[async_trait]
pub trait CameraDispatcher: Debug + Send {
    async fn dispatch(&self, frame: &Frame);
}

#[derive(Debug)]
pub struct StereoCameraDispatcher {
    l_sender: Sender<Frame>,
    r_sender: Sender<Frame>,
}

impl StereoCameraDispatcher {
    pub fn new(l_sender: Sender<Frame>, r_sender: Sender<Frame>) -> Self {
        Self { l_sender, r_sender }
    }
}

#[async_trait]
impl CameraDispatcher for StereoCameraDispatcher {
    async fn dispatch(&self, frame: &Frame) {
        let width = frame.decoded.width();
        let height = frame.decoded.height();

        let l_frame = Frame {
            timestamp: frame.timestamp,
            raw_jpeg_data: None,
            decoded: frame.decoded.view(0, 0, width / 2, height).to_image(),
        };
        let r_frame = Frame {
            timestamp: frame.timestamp,
            raw_jpeg_data: None,
            decoded: frame
                .decoded
                .view(width / 2, 0, width / 2, height)
                .to_image(),
        };

        join_all([
            self.l_sender.broadcast_direct(l_frame),
            self.r_sender.broadcast_direct(r_frame),
        ])
        .await;
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
    async fn dispatch(&self, frame: &Frame) {
        self.sender.broadcast_direct(frame.clone()).await.unwrap();
    }
}
