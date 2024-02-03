use std::{cmp::min, io::Cursor, path::Path, sync::Arc, time::SystemTime};

use imgui_wgpu::{Texture, TextureConfig};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
    join,
    net::TcpListener,
    sync::Mutex,
    task::JoinHandle,
};
use tokio_serial::SerialPortBuilderExt;

use tokio_stream::StreamExt;
use tokio_util::codec::{BytesCodec, Decoder};

mod camera;
mod camera_texture;
mod ui;
mod inference;
mod app;

use crate::{camera::*, camera_texture::CameraTexture, app::App};

#[tokio::main]
async fn main() -> tokio_serial::Result<()> {
    

    let mut app = App::new();

    let (l_camera, r_camera) = app.start_cameras("COM3".to_string(), "COM4".to_string())?;
    let ui = app.start_ui();
    let server = app.start_server();

    join!(l_camera, r_camera, ui, server);
    
    Ok(())
}
