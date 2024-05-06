use async_broadcast::broadcast;
use clap::Parser;
use frame_server::start_frame_server;
use tokio::join;

mod app;
mod camera;
mod camera_texture;
mod frame_server;
mod inference;
mod ui;

use crate::{app::App, camera::*};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Left camera URL
    #[arg(short = 'l')]
    l_camera_url: String,

    /// Right camera URL
    #[arg(short = 'r')]
    r_camera_url: String,

    /// Enable inference
    #[arg(short = 'I')]
    inference: bool,

    /// OSC output address
    #[arg(short = 'o')]
    osc_out_address: String,

    /// Path to the ONNX model
    #[arg(short = 'm', default_value = "./model.onnx")]
    model_path: String,

    /// Number of threads to use for inference per eye
    #[arg(short = 't', default_value_t = 3)]
    threads_per_eye: usize,
}

#[tokio::main]
async fn main() -> tokio_serial::Result<()> {
    let args = Args::parse();

    let (l_cam_tx, mut l_cam_rx) = broadcast::<Frame>(1);
    let (r_cam_tx, mut r_cam_rx) = broadcast::<Frame>(1);

    l_cam_rx.set_overflow(true);
    r_cam_rx.set_overflow(true);

    let mut app = App::new(l_cam_tx, r_cam_tx);

    let mut tasks = Vec::new();

    let (l_camera, r_camera) = app.start_cameras(args.l_camera_url, args.r_camera_url)?;
    let ui = app.start_ui(l_cam_rx.clone(), r_cam_rx.clone());
    let server = start_frame_server(l_cam_rx.clone(), r_cam_rx.clone());

    tasks.push(l_camera);
    tasks.push(r_camera);
    tasks.push(ui);
    tasks.push(server);

    if args.inference {
        let inference = app.start_inference(
            args.osc_out_address,
            args.model_path,
            args.threads_per_eye,
            l_cam_rx.clone(),
            r_cam_rx.clone(),
        );
        tasks.push(inference);
    }

    drop(l_cam_rx);
    drop(r_cam_rx);

    // TODO: this can be done better
    for task in tasks {
        join!(task).0.unwrap();
    }

    Ok(())
}
