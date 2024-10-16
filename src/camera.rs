use std::{
    io::Cursor,
    time::{Duration, SystemTime},
};

use async_broadcast::Sender;
use hex_literal::hex;
use hyper::http;
use image::RgbImage;
use tokio::{io::AsyncReadExt, task::JoinHandle, time::sleep};
use tokio_serial::SerialPortBuilderExt;
use tokio_stream::StreamExt;

const BAUD_RATE: u32 = 3000000;

const ETVR_PACKET_HEADER: [u8; 4] = hex!("FF A0 FF A1");

const HTTP_CONNECTION_TIMEOUT: Duration = Duration::from_secs(1);

pub const CAMERA_FRAME_SIZE: u32 = 240;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Eye {
    L,
    R,
}

#[derive(Clone)]
pub struct Frame {
    pub raw_data: Vec<u8>,
    pub decoded: RgbImage,
    pub timestamp: SystemTime,
}

pub struct Camera {
    pub eye: Eye,
    sender: Sender<Frame>,
}

impl Camera {
    pub fn new(eye: Eye, sender: Sender<Frame>) -> Camera {
        Camera { eye, sender }
    }

    pub fn start(&mut self, path: String) -> tokio_serial::Result<JoinHandle<()>> {
        if path.starts_with("COM") {
            self.connect_serial(path)
        } else {
            self.connect_http(path)
        }
    }

    fn connect_serial(&mut self, tty_path: String) -> tokio_serial::Result<JoinHandle<()>> {
        let sender = self.sender.clone();

        let future = async move {
            let mut reconnect = false;

            'init: loop {
                if reconnect {
                    println!("Reconnecting in a sec to {tty_path}");
                    sleep(Duration::from_secs(1)).await;
                }
                reconnect = true;

                let mut port = 
                match tokio_serial::new(tty_path.clone(), BAUD_RATE)
                    .open_native_async() {
                        Ok(port) => port,
                        Err(error) => {
                            println!("Serial open error: {:?}", error.kind());
                            continue 'init;
                        }
                    };
                let mut remaining_bytes = Vec::new();

                'find_packet: loop {
                    remaining_bytes.resize(remaining_bytes.len() + 2048, 0);
                    let read_position = remaining_bytes.len() - 2048;
                    match port.read_exact(&mut remaining_bytes[read_position..]).await {
                        Ok(..) => (),
                        Err(error) => {
                            println!("Serial read error: {}", error.kind());
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

                    let _ = sender.broadcast_direct(new_frame).await;
                }

                // println!("{:?} frame! {}", eye, port.bytes_to_read().unwrap());
            }
        };

        Ok(tokio::spawn(future))
    }

    fn connect_http(&mut self, url: String) -> tokio_serial::Result<JoinHandle<()>> {
        let sender = self.sender.clone();

        let future = async move {
            let mut reconnect = false;

            'connect_loop: loop {
                if reconnect {
                    println!("Reconnecting in a sec to {}", url);
                    sleep(Duration::from_secs(1)).await;
                }
                reconnect = true;

                hyper::Client::new();
                let client = hyper::Client::builder()
                    .pool_idle_timeout(HTTP_CONNECTION_TIMEOUT)
                    .build_http::<hyper::Body>();
                let result = client.get(http::Uri::try_from(url.clone()).unwrap()).await;
                if let Err(err) = result {
                    println!("{err:?}");
                    continue 'connect_loop;
                }

                let res = result.unwrap();

                if !res.status().is_success() {
                    println!("HTTP request failed with status {}", res.status());
                    continue 'connect_loop;
                }
                let content_type: mime::Mime = res
                    .headers()
                    .get(http::header::CONTENT_TYPE)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .parse()
                    .unwrap();
                assert_eq!(content_type.type_(), "multipart");
                let boundary = content_type.get_param(mime::BOUNDARY).unwrap();
                let stream = res.into_body();
                let mut stream = multipart_stream::parse(stream, boundary.as_str());
                while let Some(p) = stream.next().await {
                    let p = match p {
                        Ok(p) => p,
                        Err(err) => {
                            println!("Camera stream error:\n{err:?}");
                            continue 'connect_loop;
                        }
                    };
                    let buf = p.body;

                    let mut decoder = image::io::Reader::new(Cursor::new(buf.clone()));
                    decoder.set_format(image::ImageFormat::Jpeg);

                    let image = decoder.decode();

                    if image.is_err() {
                        println!("Failed to decode image");
                        continue;
                    }

                    let image = image.unwrap().as_rgb8().unwrap().to_owned();

                    let new_frame = Frame {
                        timestamp: SystemTime::now(),
                        raw_data: buf.to_vec(),
                        decoded: image,
                    };

                    let _ = sender.broadcast_direct(new_frame).await;
                }
            }
        };

        Ok(tokio::spawn(future))
    }
}
