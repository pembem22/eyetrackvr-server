use std::{io::Cursor, sync::Arc, time::SystemTime};

use hex_literal::hex;
use image::RgbImage;
use tokio::{io::AsyncReadExt, sync::Mutex, task::JoinHandle};
use tokio_serial::SerialPortBuilderExt;

const BAUD_RATE: u32 = 3000000;

const ETVR_PACKET_HEADER: [u8; 4] = hex!("FF A0 FF A1");

pub const CAMERA_FRAME_SIZE: u32 = 240;

#[derive(Debug, Clone, Copy)]
pub enum Eye {
    L,
    R,
}

pub struct Frame {
    pub raw_data: Vec<u8>,
    pub decoded: RgbImage,
    pub timestamp: SystemTime,
}

pub struct Camera {
    pub eye: Eye,
    pub frame: Arc<Mutex<Frame>>,
}

impl Camera {
    pub fn new(eye: Eye) -> Camera {
        Camera {
            eye,
            frame: Arc::new(Mutex::new(Frame {
                raw_data: Vec::new(),
                decoded: RgbImage::new(0, 0),
                timestamp: SystemTime::now(),
            })),
        }
    }

    pub fn start(&mut self, tty_path: String) -> tokio_serial::Result<JoinHandle<()>> {
        let frame = self.frame.clone();
        let eye = self.eye;

        let future = async move {
            'init: loop {
                let mut port = tokio_serial::new(tty_path.clone(), BAUD_RATE)
                    .open_native_async()
                    .unwrap();
                let mut remaining_bytes = Vec::new();

                'find_packet: loop {
                    remaining_bytes.resize(remaining_bytes.len() + 2048, 0);
                    let read_position = remaining_bytes.len() - 2048;
                    match port.read_exact(&mut remaining_bytes[read_position..])
                        .await {
                        Ok(..) => (),
                        Err(error) => {
                            println!("Serial error: {}", error.kind());
                            continue 'init;
                        }
                    };

                    for i in 0..remaining_bytes.len() - ETVR_PACKET_HEADER.len() - 2 + 1 {
                        if remaining_bytes[i..i + ETVR_PACKET_HEADER.len()] == ETVR_PACKET_HEADER {
                            remaining_bytes.drain(0..i);
                            break 'find_packet;
                        }
                    }
                }

                loop {
                    let mut buf = [0u8; 6];

                    let to_copy = std::cmp::min(remaining_bytes.len(), 6);
                    buf[..to_copy].copy_from_slice(&remaining_bytes[..to_copy]);
                    remaining_bytes.drain(0..to_copy);
                    match port.read_exact(&mut buf[to_copy..]).await {
                        Ok(..) => (),
                        Err(error) => {
                            println!("Serial error: {}", error.kind());
                            continue 'init;
                        }
                    };

                    if buf[0..4] != ETVR_PACKET_HEADER {
                        println!("Wrong packet header");
                        continue 'init;
                    }
                    let packet_len = u16::from_le_bytes([buf[4], buf[5]]) as usize;

                    let mut buf = vec![0; packet_len];

                    let to_copy = std::cmp::min(remaining_bytes.len(), packet_len);
                    buf[..to_copy].copy_from_slice(&remaining_bytes[..to_copy]);
                    remaining_bytes.drain(0..to_copy);
                    match port.read_exact(&mut buf[to_copy..]).await {
                        Ok(..) => (),
                        Err(error) => {
                            println!("Serial error: {}", error.kind());
                            continue 'init;
                        }
                    };

                    let mut decoder = image::io::Reader::new(Cursor::new(buf.clone()));
                    decoder.set_format(image::ImageFormat::Jpeg);

                    let image = decoder.decode();

                    if image.is_err() {
                        println!("Failed to decode image");
                        continue 'init;
                    }

                    let image = image.unwrap().as_rgb8().unwrap().to_owned();

                    let new_frame = Frame {
                        timestamp: SystemTime::now(),
                        raw_data: buf,
                        decoded: image,
                    };

                    *frame.lock().await = new_frame;
                }

                // println!("{:?} frame! {}", eye, port.bytes_to_read().unwrap());
            }
        };

        Ok(tokio::spawn(future))
    }
}
