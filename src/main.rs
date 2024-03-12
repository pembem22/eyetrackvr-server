use tokio::join;

mod app;
mod camera;
mod camera_texture;
mod inference;
mod ui;

use crate::{app::App, camera::*};

#[tokio::main]
async fn main() -> tokio_serial::Result<()> {
    let mut app = App::new();

    let (l_camera, r_camera) = app.start_cameras("http://192.168.0.227/".to_string(), "http://192.168.0.225/".to_string())?;
    let ui = app.start_ui();
    let server = app.start_server();
    let inference = app.start_inference();

    join!(l_camera, r_camera, ui, server, inference);

    Ok(())
}
