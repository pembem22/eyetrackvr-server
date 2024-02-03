use std::{sync::Arc, time::SystemTime};

use hex_literal::hex;
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
    pub data: Vec<u8>,
    pub timestamp: SystemTime,
}

pub struct Camera {
    pub eye: Eye,
    pub frame: Arc<Mutex<Frame>>,
    pub task: Option<JoinHandle<()>>,
}

impl Camera {
    pub fn new(eye: Eye) -> Camera {
        Camera {
            eye,
            frame: Arc::new(Mutex::new(Frame {
                data: Vec::new(),
                timestamp: SystemTime::now(),
            })),
            task: None,
        }
    }

    pub fn start(&mut self, tty_path: String) -> tokio_serial::Result<()> {
        let frame = self.frame.clone();
        let eye = self.eye;

        let mut port = tokio_serial::new(tty_path, BAUD_RATE).open_native_async()?;

        let future = async move {
            let mut remaining_bytes = Vec::new();
            'find_packet: loop {
                remaining_bytes.resize(remaining_bytes.len() + 2048, 0);
                let read_position = remaining_bytes.len() - 2048;
                port.read_exact(&mut remaining_bytes[read_position..])
                    .await
                    .unwrap();

                for i in 0..remaining_bytes.len() - ETVR_PACKET_HEADER.len() - 2 + 1 {
                    if remaining_bytes[i..i + ETVR_PACKET_HEADER.len()] == ETVR_PACKET_HEADER {
                        remaining_bytes.drain(0..i);
                        break 'find_packet;
                    }
                }
            }

            loop {
                let mut buf = [0u8; 6];

                if !remaining_bytes.is_empty() {
                    let to_copy = std::cmp::min(remaining_bytes.len(), 6);
                    buf[..to_copy].copy_from_slice(&remaining_bytes[..to_copy]);
                    remaining_bytes.drain(0..to_copy);
                    port.read_exact(&mut buf[to_copy..]).await.unwrap();
                } else {
                    port.read_exact(&mut buf).await.unwrap();
                }

                assert_eq!(buf[0..4], ETVR_PACKET_HEADER, "eye {:?}", eye);
                let packet_len = u16::from_le_bytes([buf[4], buf[5]]) as usize;

                let mut buf = vec![0; packet_len];

                if !remaining_bytes.is_empty() {
                    let to_copy = std::cmp::min(remaining_bytes.len(), packet_len);
                    buf[..to_copy].copy_from_slice(&remaining_bytes[..to_copy]);
                    remaining_bytes.drain(0..to_copy);
                    port.read_exact(&mut buf[to_copy..]).await.unwrap();
                } else {
                    port.read_exact(&mut buf).await.unwrap();
                }

                let new_frame = Frame {
                    timestamp: SystemTime::now(),
                    data: buf,
                };

                *frame.lock().await = new_frame;

                // println!("{:?} frame! {}", eye, port.bytes_to_read().unwrap());
            }
        };

        self.task = Some(tokio::spawn(future));

        Ok(())
    }
}
