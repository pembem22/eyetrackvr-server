use async_broadcast::broadcast;
use camera_server::start_camera_server;
use clap::Parser;
use frame_server::start_frame_server;
use futures::future::try_join_all;
#[cfg(feature = "inference")]
use inference::{start_inference, EyeState};
#[cfg(feature = "inference")]
use osc_sender::start_osc_sender;
#[cfg(feature = "gui")]
use ui::start_ui;

mod app;
mod camera;
mod camera_server;
#[cfg(feature = "gui")]
mod camera_texture;
mod frame_server;
#[cfg(feature = "inference")]
mod inference;
#[cfg(feature = "inference")]
mod osc_sender;
#[cfg(feature = "gui")]
mod ui;

use crate::{app::App, camera::*};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Left camera URL
    #[arg(short = 'l', default_value = "http://openiristracker_l.local/")]
    l_camera_url: String,

    /// Right camera URL
    #[arg(short = 'r', default_value = "http://openiristracker_r.local/")]
    r_camera_url: String,

    /// Enable inference
    #[arg(short = 'I')]
    inference: bool,

    /// OSC output address
    #[arg(short = 'o', default_value = "localhost:9000")]
    osc_out_address: String,

    /// Path to the ONNX model
    #[arg(short = 'm', default_value = "./model.onnx")]
    model_path: String,

    /// Number of threads to use for inference per eye
    #[arg(short = 't', default_value_t = 1)]
    threads_per_eye: usize,

    /// Headless mode, no GUI
    #[arg(short = 'H')]
    headless: bool,
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
    let server = start_frame_server(l_cam_rx.clone(), r_cam_rx.clone());

    tasks.push(l_camera);
    tasks.push(r_camera);
    tasks.push(server);

    if !args.headless {
        #[cfg(feature = "gui")]
        {
            let ui = start_ui(l_cam_rx.clone(), r_cam_rx.clone());
            tasks.push(ui);
        }

        #[cfg(not(feature = "inference"))]
        println!("Compiled without GUI support, starting headless anyway")
    }

    if args.inference {
        #[cfg(feature = "inference")]
        {
            let (raw_eyes_tx, raw_eyes_rx) = broadcast::<(EyeState, EyeState)>(1);

            let inference = start_inference(
                l_cam_rx.clone(),
                r_cam_rx.clone(),
                raw_eyes_tx.clone(),
                args.model_path,
                args.threads_per_eye,
            );
            tasks.push(inference);

            let osc_sender = start_osc_sender(raw_eyes_rx.clone(), args.osc_out_address);
            tasks.push(osc_sender);

            drop(raw_eyes_rx);
        }

        #[cfg(not(feature = "inference"))]
        println!("Compiled without inference support, ignoring")
    }

    let camera_server = start_camera_server(l_cam_rx.clone(), r_cam_rx.clone());
    tasks.push(camera_server);

    drop(l_cam_rx);
    drop(r_cam_rx);

    let _ = try_join_all(tasks).await.unwrap();

    Ok(())
}
