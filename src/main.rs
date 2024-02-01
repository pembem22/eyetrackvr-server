use std::{
    cmp::min,
    collections::HashMap,
    env,
    io::Cursor,
    str,
    sync::{Arc, Mutex as StdMutex},
    time::{Duration, SystemTime},
};

use imgui::TextureId;
use imgui_wgpu::{Texture, TextureConfig};
use tokio::{io::AsyncReadExt, join, sync::Mutex, task::JoinHandle};
use tokio_serial::{SerialPort, SerialPortBuilderExt};

use hex_literal::hex;
use wgpu::Extent3d;

use image::{codecs::jpeg::JpegDecoder, ImageDecoder};

#[cfg(unix)]
const DEFAULT_TTY: &str = "/dev/ttyUSB0";
#[cfg(windows)]
const DEFAULT_TTY: &str = "COM5";

const BAUD_RATE: u32 = 3000000;

const ETVR_PACKET_HEADER: [u8; 4] = hex!("FF A0 FF A1");

const CAMERA_FRAME_SIZE: u32 = 240;

struct Frame {
    data: Vec<u8>,
    timestamp: SystemTime,
}

struct Camera {
    eye: Eye,
    frame: Arc<Mutex<Frame>>,
    task: Option<JoinHandle<()>>,
}

impl Camera {
    fn new(eye: Eye) -> Camera {
        Camera {
            eye,
            frame: Arc::new(Mutex::new(Frame {
                data: Vec::new(),
                timestamp: SystemTime::now(),
            })),
            task: None,
        }
    }

    fn start(&mut self, tty_path: String) -> tokio_serial::Result<()> {
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
                    let to_copy = min(remaining_bytes.len(), 6);
                    buf[..to_copy].copy_from_slice(&remaining_bytes[..to_copy]);
                    remaining_bytes.drain(0..to_copy);
                    port.read_exact(&mut buf[to_copy..]).await.unwrap();
                } else {
                    port.read_exact(&mut buf).await.unwrap();
                }

                assert_eq!(buf[0..4], ETVR_PACKET_HEADER);
                let packet_len = u16::from_le_bytes([buf[4], buf[5]]) as usize;

                let mut buf = vec![0; packet_len];

                if !remaining_bytes.is_empty() {
                    let to_copy = min(remaining_bytes.len(), packet_len);
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

#[derive(Debug, Clone, Copy)]
enum Eye {
    L,
    R,
}

mod ui;

#[tokio::main]
async fn main() -> tokio_serial::Result<()> {
    let mut l_camera = Camera::new(Eye::L);
    // let mut r_camera = Camera::new(Eye::R);

    l_camera.start("COM3".to_string())?;
    // r_camera.start("COM4".to_string())?;

    let ui_task = tokio::task::spawn_blocking(|| {

        let mut ui = ui::UI::new();

        let texture_config: TextureConfig<'_> = TextureConfig {
            size: wgpu::Extent3d {
                width: CAMERA_FRAME_SIZE,
                height: CAMERA_FRAME_SIZE,
                ..Default::default()
            },
            label: Some("lenna texture"),
            format: Some(wgpu::TextureFormat::Rgba8Unorm),
            ..Default::default()
        };

        let texture = Texture::new(&ui.device, &ui.renderer, texture_config);
        let l_texture_id = ui.renderer.textures.insert(texture);

        ui.run(move |ui, queue, renderer| {
            let jpeg_data = l_camera.frame.blocking_lock().data.clone();

            let mut decoder = image::io::Reader::new(Cursor::new(jpeg_data));
            decoder.set_format(image::ImageFormat::Jpeg);
            let image = decoder.decode().unwrap();
            // println!("{:?}", decoder.);

            renderer.textures.get(l_texture_id).unwrap().write(
                &queue,
                &image.into_rgba8(),
                CAMERA_FRAME_SIZE,
                CAMERA_FRAME_SIZE,
            );

            ui.window("Hello!").build(|| {
              imgui::Image::new(l_texture_id, [CAMERA_FRAME_SIZE as f32, CAMERA_FRAME_SIZE as f32]).build(ui)
            });
        });
    });

    join!(l_camera.task.unwrap(), ui_task);
    Ok(())
}
