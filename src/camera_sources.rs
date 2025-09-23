use std::fmt::Debug;
use std::{
    io::Cursor,
    time::{Duration, SystemTime},
};

use futures::StreamExt;

use hyper::http;
use nokhwa::utils::CameraFormat;
use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
};
use pollster::FutureExt;
use tokio::task::JoinHandle;

use crate::camera::Frame;
use crate::camera_dispatcher::CameraDispatcher;

#[derive(Clone, Debug)]
struct FpsCounter {
    last_second: SystemTime,
    frames_since_last_second: u32,
}

impl FpsCounter {
    fn new() -> Self {
        Self {
            last_second: SystemTime::now(),
            frames_since_last_second: 0,
        }
    }

    fn update_fps(&mut self) {
        let now = SystemTime::now();
        if now.duration_since(self.last_second).unwrap().as_secs() > 0 {
            println!("FPS: {}", self.frames_since_last_second);

            self.last_second = now;
            self.frames_since_last_second = 0;
        }

        self.frames_since_last_second += 1;
    }
}

pub trait CameraSource {
    fn run(&self, dispatcher: Box<dyn CameraDispatcher>) -> JoinHandle<()>;
}

const HTTP_CONNECTION_TIMEOUT: Duration = Duration::from_secs(1);
#[derive(Clone, Debug)]
pub struct HttpCameraSource {
    url: String,
}

impl HttpCameraSource {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

impl CameraSource for HttpCameraSource {
    fn run(&self, dispatcher: Box<dyn CameraDispatcher>) -> JoinHandle<()> {
        let url = self.url.clone();
        // let dispatcher = dispatcher.clone();

        let future = async move {
            let mut reconnect = false;

            'connect_loop: loop {
                if reconnect {
                    println!("Reconnecting in a sec to {url}");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                reconnect = true;

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

                    let mut decoder = image::ImageReader::new(Cursor::new(buf.clone()));
                    decoder.set_format(image::ImageFormat::Jpeg);

                    let image = decoder.decode();

                    if image.is_err() {
                        println!("Failed to decode image");
                        continue;
                    }

                    let frame = Frame {
                        timestamp: SystemTime::now(),
                        raw_jpeg_data: Some(buf.to_vec()),
                        decoded: image.unwrap().into_rgb8(),
                    };

                    dispatcher.dispatch(frame).await;
                }
            }
        };
        tokio::spawn(future)
    }
}

#[derive(Clone, Debug)]
pub struct UvcCameraSource {
    uvc_index: u32,
}

impl UvcCameraSource {
    pub fn new(uvc_index: u32) -> Self {
        Self { uvc_index }
    }
}

impl CameraSource for UvcCameraSource {
    fn run(&self, dispatcher: Box<dyn CameraDispatcher>) -> JoinHandle<()> {
        let uvc_index = self.uvc_index;
        let mut fps = FpsCounter::new();

        let future = move || {
            let index = CameraIndex::Index(uvc_index);
            let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Exact(
                CameraFormat::new_from(320, 240, nokhwa::utils::FrameFormat::MJPEG, 120),
            ));
            println!("Requested format: {requested:?}");
            let mut camera = nokhwa::Camera::new(index, requested).unwrap();

            // Docs say this is required, but not calling it also works lmao whatever, that lib is cooked.
            camera.open_stream().unwrap();

            println!(
                "Connected to a UVC camera fps:{} {:?}",
                camera.frame_rate(),
                camera.frame_format()
            );

            while let Ok(frame_raw) = camera.frame_raw() {
                let mut decoder = image::ImageReader::new(Cursor::new(&frame_raw));
                decoder.set_format(image::ImageFormat::Jpeg);

                let image = decoder.decode();

                if image.is_err() {
                    println!("Failed to decode image");
                    continue;
                }

                let frame = Frame {
                    timestamp: SystemTime::now(),
                    raw_jpeg_data: Some(Vec::from(frame_raw)),
                    decoded: image.unwrap().into_rgb8(),
                };

                dispatcher.dispatch(frame).block_on();

                fps.update_fps();
            }
        };
        tokio::task::spawn_blocking(future)
    }
}

#[derive(Clone, Debug)]
// TODO: implement
pub struct SerialCameraSource {}

// fn connect_serial(&self, tty_path: String) -> JoinHandle<()> {
//     let sender = self.sender.clone();

//     let future = async move {
//         let mut reconnect = false;

//         'connect_loop: loop {
//             if reconnect {
//                 println!("Reconnecting in a sec to {tty_path}");
//                 sleep(Duration::from_secs(1)).await;
//             }
//             reconnect = true;

//             let mut port =
//                 match tokio_serial::new(tty_path.clone(), BAUD_RATE).open_native_async() {
//                     Ok(port) => port,
//                     Err(error) => {
//                         println!("Serial open error: {error:?}");
//                         // return;
//                         continue 'connect_loop;
//                     }
//                 };
//             let mut remaining_bytes = Vec::new();
//             'init: loop {
//                 'find_packet: loop {
//                     remaining_bytes.resize(remaining_bytes.len() + 2048, 0);
//                     let read_position = remaining_bytes.len() - 2048;

//                     match port.read_exact(&mut remaining_bytes[read_position..]).await {
//                         Ok(..) => (),
//                         Err(error) => {
//                             println!("Serial read error: {error:?}");
//                             // TODO: deduplicate
//                             if let Some(raw_err) = error.raw_os_error() {
//                                 if raw_err == 22 {
//                                     continue 'connect_loop;
//                                 }
//                             }
//                             continue 'init;
//                         }
//                     };

//                     for i in 0..remaining_bytes.len() - ETVR_PACKET_HEADER.len() - 2 + 1 {
//                         if remaining_bytes[i..i + ETVR_PACKET_HEADER.len()]
//                             == ETVR_PACKET_HEADER
//                         {
//                             remaining_bytes.drain(0..i);
//                             break 'find_packet;
//                         }
//                     }
//                 }

//                 loop {
//                     let mut buf = [0u8; 6];

//                     let to_copy = std::cmp::min(remaining_bytes.len(), 6);
//                     buf[..to_copy].copy_from_slice(&remaining_bytes[..to_copy]);
//                     remaining_bytes.drain(0..to_copy);
//                     match port.read_exact(&mut buf[to_copy..]).await {
//                         Ok(..) => (),
//                         Err(error) => {
//                             println!("Warning: failed to read exact frame: {error:?}");
//                             // continue 'init;
//                         }
//                     };

//                     if buf[0..4] != ETVR_PACKET_HEADER {
//                         println!("Wrong packet header");
//                         continue 'init;
//                     }
//                     let packet_len = u16::from_le_bytes([buf[4], buf[5]]) as usize;

//                     let mut buf = vec![0; packet_len];

//                     let to_copy = std::cmp::min(remaining_bytes.len(), packet_len);
//                     buf[..to_copy].copy_from_slice(&remaining_bytes[..to_copy]);
//                     remaining_bytes.drain(0..to_copy);
//                     match port.read_exact(&mut buf[to_copy..]).await {
//                         Ok(..) => (),
//                         Err(error) => {
//                             println!("Warning: failed to read exact frame: {error:?}");
//                             // TODO: deduplicate
//                             if let Some(raw_err) = error.raw_os_error() {
//                                 if raw_err == 22 {
//                                     continue 'connect_loop;
//                                 }
//                             }
//                             // continue 'init;
//                         }
//                     };

//                     let mut decoder = image::ImageReader::new(Cursor::new(buf.clone()));
//                     decoder.set_format(image::ImageFormat::Jpeg);

//                     let image = decoder.decode();

//                     if image.is_err() {
//                         println!("Warning: failed to decode image: {:?}", image.unwrap_err());
//                         continue;
//                     }

//                     let image = image.unwrap().as_rgb8().unwrap().to_owned();

//                     let new_frame = Frame {
//                         timestamp: SystemTime::now(),
//                         raw_jpeg_data: Some(buf),
//                         decoded: image,
//                     };

//                     let _ = sender.broadcast_direct(new_frame).await;
//                 }

//                 // println!("{:?} frame! {}", eye, port.bytes_to_read().unwrap());
//             }
//         }
//     };

//     tokio::spawn(future)
// }