use async_broadcast::broadcast;
// use camera_server::start_camera_server;
use clap::Parser;
use data_processing::{filter_eye, merge_eyes};
use frame_server::start_frame_server;
use futures::future::try_join_all;
#[cfg(feature = "inference")]
use inference::EyeState;
use inference::eye_inference;
#[cfg(feature = "inference")]
use osc_sender::start_osc_sender;
use tokio::task::JoinHandle;
#[cfg(feature = "gui")]
use window::start_ui;

mod app;
mod camera;
// mod camera_server;
#[cfg(feature = "gui")]
mod camera_texture;
mod data_processing;
mod frame_server;
#[cfg(feature = "inference")]
mod inference;
#[cfg(feature = "inference")]
mod osc_sender;
#[cfg(feature = "gui")]
mod ui;
#[cfg(feature = "gui")]
mod window;

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

    /// Face camera URL
    #[arg(short = 'f', default_value = "http://openiristracker_face.local/")]
    f_camera_url: String,

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

    let tasks = configure_tasks(&args)?;

    let _ = try_join_all(tasks).await.unwrap();

    Ok(())
}

fn configure_tasks(args: &Args) -> tokio_serial::Result<Vec<JoinHandle<()>>> {
    let mut tasks = Vec::new();

    let (l_cam_tx, mut l_cam_rx) = broadcast::<Frame>(1);
    let (r_cam_tx, mut r_cam_rx) = broadcast::<Frame>(1);
    let (f_cam_tx, mut f_cam_rx) = broadcast::<Frame>(1);

    l_cam_rx.set_overflow(true);
    r_cam_rx.set_overflow(true);
    f_cam_rx.set_overflow(true);

    let mut app = App::new(l_cam_tx, r_cam_tx, f_cam_tx);

    // Connect to cameras

    let (l_camera, r_camera, f_camera) = app.start_cameras(
        args.l_camera_url.clone(),
        args.r_camera_url.clone(),
        args.f_camera_url.clone(),
    )?;
    tasks.push(l_camera);
    tasks.push(r_camera);
    tasks.push(f_camera);

    // Save dataset

    let server = start_frame_server(l_cam_rx.clone(), r_cam_rx.clone());

    tasks.push(server);

    // Inference

    let (l_raw_eye_tx, mut l_raw_eye_rx) = broadcast::<EyeState>(1);
    let (r_raw_eye_tx, mut r_raw_eye_rx) = broadcast::<EyeState>(1);
    l_raw_eye_rx.set_overflow(true);
    r_raw_eye_rx.set_overflow(true);

    if args.inference {
        #[cfg(feature = "inference")]
        {
            tasks.push(eye_inference(
                l_cam_rx.clone(),
                &args.model_path,
                args.threads_per_eye,
                l_raw_eye_tx,
                Eye::L,
            ));
            tasks.push(eye_inference(
                r_cam_rx.clone(),
                &args.model_path,
                args.threads_per_eye,
                r_raw_eye_tx,
                Eye::R,
            ));
        }

        #[cfg(not(feature = "inference"))]
        println!("Compiled without inference support, ignoring")
    }

    // Filter

    let (l_filtered_eye_tx, mut l_filtered_eye_rx) = broadcast::<EyeState>(1);
    let (r_filtered_eye_tx, mut r_filtered_eye_rx) = broadcast::<EyeState>(1);
    l_filtered_eye_rx.set_overflow(true);
    r_filtered_eye_rx.set_overflow(true);

    tasks.push(filter_eye(l_raw_eye_rx.clone(), l_filtered_eye_tx));
    tasks.push(filter_eye(r_raw_eye_rx.clone(), r_filtered_eye_tx));

    // Merge

    let (filtered_eyes_tx, mut filtered_eyes_rx) = broadcast::<(EyeState, EyeState)>(1);
    filtered_eyes_rx.set_overflow(true);

    tasks.push(merge_eyes(
        l_filtered_eye_rx.clone(),
        r_filtered_eye_rx.clone(),
        filtered_eyes_tx,
    ));

    // OSC sender

    tasks.push(start_osc_sender(
        filtered_eyes_rx.clone(),
        args.osc_out_address.clone(),
    ));

    // GUI

    if !args.headless {
        #[cfg(feature = "gui")]
        {
            use crate::ui::AppRendererContext;

            // FIXME: rn blocks the thread here.
            start_ui(AppRendererContext {
                l_rx: l_cam_rx.clone(),
                r_rx: r_cam_rx.clone(),
                f_rx: f_cam_rx.clone(),
                l_raw_rx: l_raw_eye_rx.clone(),
                r_raw_rx: r_raw_eye_rx.clone(),
                filtered_eyes_rx: filtered_eyes_rx.clone(),
            });
            // tasks.push(ui);
        }

        #[cfg(not(feature = "inference"))]
        println!("Compiled without GUI support, starting headless anyway")
    }

    // HTTP server to mirror cameras
    // let camera_server = start_camera_server(l_cam_rx.clone(), f_cam_rx.clone());
    // tasks.push(camera_server);

    Ok(tasks)
}
