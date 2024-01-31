use std::{
    collections::HashMap,
    env, str,
    sync::Arc,
    sync::Mutex as StdMutex,
    time::{Duration, SystemTime},
};

use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt},
    net::UdpSocket,
    sync::Mutex,
    task::JoinHandle,
};
use tokio_serial::SerialPortBuilderExt;

use hex_literal::hex;

#[cfg(unix)]
const DEFAULT_TTY: &str = "/dev/ttyUSB0";
#[cfg(windows)]
const DEFAULT_TTY: &str = "COM5";

const BAUD_RATE: u32 = 3000000;

const ETVR_PACKET_HEADER: [u8; 4] = hex!("FF A0 FF A1");

struct Camera {
    eye: Eye,
    frame: Arc<Mutex<Vec<u8>>>,
    frame_time: SystemTime,
    task: Option<JoinHandle<()>>,
}

impl Camera {
    fn new(eye: Eye) -> Camera {
        let frame: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));

        Camera {
            eye,
            frame,
            frame_time: SystemTime::now(),
            task: None,
        }
    }

    fn start(mut self, tty_path: String) -> tokio_serial::Result<()> {
        let mut port = tokio_serial::new(tty_path, BAUD_RATE).open_native_async()?;

        let future = async move {
            let mut vec = Vec::new();
            'find_packet: loop {
                let mut buf = vec![0; 2048];
                port.read_exact(&mut buf).await.unwrap();

                vec.append(&mut buf);
                drop(buf);

                for i in 0..vec.len() - ETVR_PACKET_HEADER.len() - 2 + 1 {
                    if vec[i..i + ETVR_PACKET_HEADER.len()] == ETVR_PACKET_HEADER {
                        let len = u16::from_le_bytes([vec[i + 4], vec[i + 5]]);
                        vec.drain(0..i + 6);
                        vec.resize(len as usize, 0);
                        port.read_exact(&mut vec).await.unwrap();
                        let mut frame_vec = self.frame.lock().await;
                        self.frame_time = SystemTime::now();
                        *frame_vec = vec;
                        break 'find_packet;
                    }
                }
            }

            loop {
                let mut buf = [0u8; 6];
                port.read_exact(&mut buf).await.unwrap();
                let len = u16::from_le_bytes([buf[4], buf[5]]);

                let mut vec = vec![0; len as usize];
                port.read_exact(&mut vec).await.unwrap();
                self.frame_time = SystemTime::now();
                let mut frame_vec = self.frame.lock().await;
                *frame_vec = vec;
            }
        };

        self.task = Some(tokio::spawn(future));

        Ok(())
    }
}

enum Eye {
    L,
    R,
}

#[tokio::main]
async fn main() -> tokio_serial::Result<()> {
    let l_camera = Camera::new(Eye::L);
    let r_camera = Camera::new(Eye::R);

    l_camera.start("COM3".to_string())?;
    r_camera.start("COM4".to_string())?;

    Ok(())
}
