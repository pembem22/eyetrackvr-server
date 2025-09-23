use std::fmt::Debug;
use std::{
    io::Cursor,
    time::{Duration, SystemTime},
};

use async_broadcast::Sender;
use hex_literal::hex;
use hyper::http;
use image::codecs::jpeg::JpegEncoder;
use image::{GenericImageView, RgbImage};
use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
};
use tokio::{io::AsyncReadExt, task::JoinHandle, time::sleep};
use tokio_serial::SerialPortBuilderExt;
use tokio_stream::StreamExt;

const BAUD_RATE: u32 = 3000000;

const ETVR_PACKET_HEADER: [u8; 4] = hex!("FF A0 FF A1");

const HTTP_CONNECTION_TIMEOUT: Duration = Duration::from_secs(1);

pub const CAMERA_FRAME_SIZE: u32 = 240;

#[derive(Clone, Debug)]
pub struct Frame {
    pub raw_jpeg_data: Option<Vec<u8>>,
    pub decoded: RgbImage,
    pub timestamp: SystemTime,
}

impl<'a> Frame {
    pub fn as_jpeg_bytes(&mut self) -> Vec<u8> {
        if self.raw_jpeg_data.is_none() {
            let vec = Vec::with_capacity(8192);
            let mut cursor = Cursor::new(vec);

            JpegEncoder::new(&mut cursor)
                .encode_image(&self.decoded)
                .unwrap();
            self.raw_jpeg_data = Some(cursor.into_inner());
        }

        self.raw_jpeg_data.clone().unwrap()
    }
}
